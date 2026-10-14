//! Background-friendly directory search, independent of the native frontend.
use crate::{Document, Encoding, SearchOptions};
use std::{
    collections::HashMap,
    io,
    ops::Range,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub struct FileSearchQuery {
    pub needle: String,
    pub directory: PathBuf,
    pub filters: String,
    pub recursive: bool,
    pub include_hidden: bool,
    pub regex: bool,
    pub dot_matches_newline: bool,
    pub options: SearchOptions,
}
#[derive(Clone, Debug)]
pub struct FileSearchHit {
    pub path: PathBuf,
    pub encoding: Encoding,
    pub range: Range<usize>,
    pub line: usize,
    pub preview: String,
}
#[derive(Debug)]
pub enum FileSearchEvent {
    Progress(PathBuf, usize),
    Hits(Vec<FileSearchHit>),
    Error(PathBuf, String),
    Finished {
        scanned: usize,
        matching_files: usize,
        hits: usize,
        cancelled: bool,
    },
}
pub type OpenFileContents = HashMap<PathBuf, (String, Encoding)>;

/// Compile before starting a worker so malformed expressions can be reported
/// immediately. The regex dialect supports look-around and backreferences,
/// though it does not yet reproduce every Boost/Scintilla extension.
pub fn validate_file_search(query: &FileSearchQuery) -> io::Result<()> {
    if query.needle.is_empty() {
        return Err(io::Error::other("Enter text to find"));
    }
    if !query.directory.is_dir() {
        return Err(io::Error::other("The search directory does not exist"));
    }
    expression(query)?;
    query
        .options
        .mode
        .decode(&query.needle)
        .map_err(io::Error::other)?;
    Ok(())
}
fn expression(query: &FileSearchQuery) -> io::Result<Option<fancy_regex::Regex>> {
    if !query.regex {
        return Ok(None);
    }
    fancy_regex::RegexBuilder::new(&query.needle)
        .case_insensitive(!query.options.match_case)
        .multi_line(true)
        .dot_matches_new_line(query.dot_matches_newline)
        .build()
        .map(Some)
        .map_err(io::Error::other)
}
fn wildcard(pattern: &str, text: &str) -> bool {
    let pattern = if pattern == "*.*" { "*" } else { pattern };
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let (mut p, mut t, mut star, mut retry) = (0, 0, None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            retry = t;
        } else if let Some(s) = star {
            retry += 1;
            t = retry;
            p = s + 1;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}
fn hidden(path: &Path, metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let _ = path;
        metadata.file_attributes() & 2 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        path.file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'))
    }
}
pub fn search_files(
    query: FileSearchQuery,
    open_files: OpenFileContents,
    cancel: &AtomicBool,
    mut emit: impl FnMut(FileSearchEvent),
) {
    let regex = match expression(&query) {
        Ok(regex) => regex,
        Err(error) => {
            emit(FileSearchEvent::Error(
                query.directory.clone(),
                error.to_string(),
            ));
            return;
        }
    };
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    let mut directories = Vec::new();
    for pattern in query.filters.split_whitespace() {
        if let Some(pattern) = pattern.strip_prefix("!+\\") {
            directories.push((pattern, true));
        } else if let Some(pattern) = pattern.strip_prefix("!\\") {
            directories.push((pattern, false));
        } else if let Some(pattern) = pattern.strip_prefix('!') {
            exclude.push(pattern);
        } else {
            include.push(pattern);
        }
    }
    if include.is_empty() {
        include.push("*.*");
    }
    let mut pending = vec![(query.directory.clone(), 0)];
    let (mut scanned, mut matching_files, mut total) = (0, 0, 0);
    while let Some((directory, depth)) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                emit(FileSearchEvent::Error(directory, error.to_string()));
                continue;
            }
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    emit(FileSearchEvent::Error(path, e.to_string()));
                    continue;
                }
            };
            if metadata.is_dir() && !query.include_hidden && hidden(&path, &metadata) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if metadata.is_dir() {
                // Reparse/symlink directories are not followed, preventing loops.
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    metadata.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let linked = entry.file_type().is_ok_and(|t| t.is_symlink());
                if query.recursive
                    && !linked
                    && !directories
                        .iter()
                        .any(|(pattern, all)| (*all || depth == 0) && wildcard(pattern, &name))
                {
                    pending.push((path, depth + 1));
                }
                continue;
            }
            if !metadata.is_file()
                || !include.iter().any(|p| wildcard(p, &name))
                || exclude.iter().any(|p| wildcard(p, &name))
            {
                continue;
            }
            scanned += 1;
            if scanned == 1 || scanned % 32 == 0 {
                emit(FileSearchEvent::Progress(path.clone(), scanned));
            }
            let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let result = (|| -> io::Result<Vec<FileSearchHit>> {
                let (text, encoding) = if let Some(contents) = open_files.get(&canonical) {
                    contents.clone()
                } else {
                    let bytes = std::fs::read(&path)?;
                    let (encoding, text) = Encoding::detect_file(&bytes)
                        .or_else(|_| {
                            Encoding::Windows1252
                                .decode_as(&bytes)
                                .map(|text| (Encoding::Windows1252, text))
                        })
                        .map_err(io::Error::other)?;
                    (text, encoding)
                };
                // Executables, object files and other binaries otherwise produce
                // unreadable previews. Occasional escape/control characters in
                // text logs are retained and escaped by the results view.
                let sample: Vec<_> = text.chars().take(8192).collect();
                let controls = sample
                    .iter()
                    .filter(|&&c| {
                        c.is_control() && !matches!(c, '\t' | '\r' | '\n' | '\u{c}' | '\u{1b}')
                    })
                    .count();
                if text.contains('\0') || (controls > 4 && controls * 100 > sample.len()) {
                    return Ok(Vec::new());
                }
                let doc = Document::from_bytes(text.as_bytes()).map_err(io::Error::other)?;
                let ranges = if let Some(regex) = &regex {
                    let mut ranges = Vec::new();
                    for found in regex.find_iter(&text) {
                        if cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        let found = found.map_err(io::Error::other)?;
                        ranges.push(found.start()..found.end());
                    }
                    ranges
                } else {
                    let needle = crate::search::prepare_needle(&query.needle, query.options)
                        .map_err(io::Error::other)?;
                    let mut ranges = Vec::new();
                    let (mut start, mut next_match) = (0, 0);
                    while start < text.len() && !cancel.load(Ordering::Relaxed) {
                        let mut end = (start + 65536).min(text.len());
                        while !text.is_char_boundary(end) {
                            end -= 1;
                        }
                        for range in
                            crate::search::match_ranges(&text, &needle, query.options, start..end)
                        {
                            if range.start >= next_match {
                                next_match = range.end;
                                ranges.push(range);
                            }
                        }
                        start = end;
                    }
                    ranges
                };
                Ok(ranges
                    .into_iter()
                    .map(|range| {
                        let line = doc
                            .line_starts()
                            .partition_point(|&start| start <= range.start)
                            .saturating_sub(1);
                        let preview = doc
                            .line_range(line)
                            .map(|range| {
                                text[range]
                                    .trim_end_matches(['\r', '\n'])
                                    .chars()
                                    .take(500)
                                    .collect()
                            })
                            .unwrap_or_default();
                        FileSearchHit {
                            path: canonical.clone(),
                            encoding,
                            range,
                            line,
                            preview,
                        }
                    })
                    .collect())
            })();
            match result {
                Ok(hits) if !hits.is_empty() => {
                    matching_files += 1;
                    total += hits.len();
                    emit(FileSearchEvent::Hits(hits));
                }
                Ok(_) => (),
                Err(error) => emit(FileSearchEvent::Error(path, error.to_string())),
            }
        }
    }
    emit(FileSearchEvent::Finished {
        scanned,
        matching_files,
        hits: total,
        cancelled: cancel.load(Ordering::Relaxed),
    });
}
