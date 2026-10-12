//! Notepad++ session XML. File attributes retain source position/encoding and
//! backup metadata even when the current frontend cannot apply every field.
use crate::{Document, Encoding, Selection, Workspace};
use quick_xml::{
    Reader, Writer,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};
use std::{collections::BTreeMap, io};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionNode {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub children: Vec<SessionNode>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionFile {
    pub attributes: BTreeMap<String, String>,
    pub bookmarks: Vec<usize>,
    pub folds: Vec<usize>,
    pub extra_nodes: Vec<SessionNode>,
}
impl SessionFile {
    /// An explicit legacy code page overrides automatic Unicode detection.
    /// Refuse unsupported code pages instead of silently opening mojibake.
    pub fn explicit_encoding(&self) -> io::Result<Option<Encoding>> {
        let codepage = self
            .attributes
            .get("encoding")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(-1);
        match codepage {
            -1 => Ok(None),
            1252 => Ok(Some(Encoding::Windows1252)),
            1251 => Ok(Some(Encoding::Windows1251)),
            932 => Ok(Some(Encoding::ShiftJis)),
            _ => Err(invalid(format!("Unsupported session encoding: {codepage}"))),
        }
    }

    /// Open a named file from disk and apply its saved editor state. Snapshot
    /// backups are deliberately handled separately from a manual session load.
    pub fn open_document(&self) -> io::Result<Document> {
        let filename = self
            .filename()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| invalid("Session file has no filename"))?;
        let mut document = Document::open_as(filename, self.explicit_encoding()?)?;
        self.restore_document_state(&mut document)?;
        Ok(document)
    }

    /// Apply selection, user read-only state and additive bookmarks without
    /// changing text or undo history. Invalid/outdated positions are clamped to
    /// valid character boundaries; invalid marker lines are ignored.
    pub fn restore_document_state(&self, document: &mut Document) -> io::Result<()> {
        let position = |key: &str| -> io::Result<usize> {
            let stored = self
                .attributes
                .get(key)
                .and_then(|value| value.parse::<i64>().ok())
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0);
            if matches!(
                document.encoding(),
                Encoding::Windows1252 | Encoding::Windows1251 | Encoding::ShiftJis
            ) {
                let mut encoded_offset = 0usize;
                for (offset, ch) in document.text().char_indices() {
                    let mut buffer = [0; 4];
                    let length = document
                        .encoding()
                        .encode(ch.encode_utf8(&mut buffer))
                        .map_err(io::Error::other)?
                        .len();
                    if stored < encoded_offset + length {
                        return Ok(offset);
                    }
                    encoded_offset += length;
                }
                Ok(document.text().len())
            } else {
                let mut offset = stored.min(document.text().len());
                while !document.text().is_char_boundary(offset) {
                    offset -= 1;
                }
                Ok(offset)
            }
        };
        // Resolve both endpoints before mutating any state.
        let selection = Selection {
            anchor: position("startPos")?,
            caret: position("endPos")?,
        };
        document.select(selection).map_err(io::Error::other)?;
        document.set_read_only(
            self.attributes
                .get("userReadOnly")
                .is_some_and(|value| value == "yes"),
        );
        let line_count = document.line_starts().len();
        document.bookmarks.0.extend(
            self.bookmarks
                .iter()
                .copied()
                .filter(|&line| line < line_count),
        );
        Ok(())
    }

    pub fn from_document(document: &Document) -> io::Result<Option<Self>> {
        let Some(path) = document.path() else {
            return Ok(None);
        };
        let filename = path
            .to_str()
            .ok_or_else(|| invalid("Session filename cannot be represented as Unicode"))?;
        let mut file = Self::new(filename.into());
        let codepage = match document.encoding() {
            Encoding::Windows1252 => 1252,
            Encoding::Windows1251 => 1251,
            Encoding::ShiftJis => 932,
            _ => -1,
        };
        let position = |offset: usize| -> io::Result<usize> {
            if codepage == -1 {
                Ok(offset)
            } else {
                document
                    .encoding()
                    .encode(&document.text()[..offset])
                    .map(|bytes| bytes.len())
                    .map_err(io::Error::other)
            }
        };
        for (key, value) in [
            (
                "startPos",
                position(document.selection().anchor)?.to_string(),
            ),
            ("endPos", position(document.selection().caret)?.to_string()),
            ("encoding", codepage.to_string()),
            (
                "userReadOnly",
                if document.is_user_read_only() {
                    "yes"
                } else {
                    "no"
                }
                .into(),
            ),
        ] {
            file.attributes.insert(key.into(), value);
        }
        for (key, value) in [
            ("firstVisibleLine", "0"),
            ("xOffset", "0"),
            ("selMode", "0"),
            ("lang", "Normal Text"),
        ] {
            file.attributes.insert(key.into(), value.into());
        }
        file.bookmarks = document.bookmarked_lines().collect();
        Ok(Some(file))
    }
    pub fn new(filename: String) -> Self {
        Self {
            attributes: BTreeMap::from([("filename".into(), filename)]),
            ..Self::default()
        }
    }
    pub fn filename(&self) -> Option<&str> {
        self.attributes.get("filename").map(String::as_str)
    }
}

impl Workspace {
    /// Capture named buffers in current tab order without saving document text.
    /// Untitled/recovery sessions require a separate backup-persistence path.
    pub fn capture_session(&self) -> io::Result<Session> {
        let mut session = Session::default();
        for (index, (id, document)) in self.buffers().enumerate() {
            if Some(id) == self.active() {
                session.main_view.active_index = index;
            }
            if let Some(file) = SessionFile::from_document(document)? {
                session.main_view.files.push(file);
            }
        }
        Ok(session)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionView {
    pub active_index: usize,
    pub files: Vec<SessionFile>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    pub active_view: usize,
    pub main_view: SessionView,
    pub sub_view: SessionView,
    pub extra_nodes: Vec<SessionNode>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn number(node: &SessionNode, key: &str, default: usize) -> usize {
    node.attributes
        .get(key)
        .and_then(|value| value.parse::<i32>().ok())
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(default)
}
fn parse_xml(bytes: &[u8]) -> io::Result<SessionNode> {
    let xml = std::str::from_utf8(bytes).map_err(|e| invalid(e.to_string()))?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().expand_empty_elements = true;
    let mut stack: Vec<SessionNode> = Vec::new();
    let mut root = None;
    loop {
        match reader.read_event().map_err(|e| invalid(e.to_string()))? {
            Event::Start(event) | Event::Empty(event) => {
                // Empty elements are expanded so both forms follow one path.
                let mut node = SessionNode {
                    name: std::str::from_utf8(event.name().as_ref())
                        .map_err(|e| invalid(e.to_string()))?
                        .into(),
                    ..SessionNode::default()
                };
                for attribute in event.attributes() {
                    let attribute = attribute.map_err(|e| invalid(e.to_string()))?;
                    let key = std::str::from_utf8(attribute.key.as_ref())
                        .map_err(|e| invalid(e.to_string()))?;
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|e| invalid(e.to_string()))?;
                    node.attributes.insert(key.into(), value.into_owned());
                }
                if stack.len() >= 64 {
                    return Err(invalid("Session XML nesting is too deep"));
                }
                stack.push(node);
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| invalid("Unexpected closing element"))?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else if root.replace(node).is_some() {
                    return Err(invalid("Multiple session roots"));
                }
            }
            Event::Eof => break,
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => (),
            Event::Text(text) if text.as_ref().iter().all(u8::is_ascii_whitespace) => (),
            _ => return Err(invalid("Unsupported content in session XML")),
        }
    }
    if !stack.is_empty() {
        return Err(invalid("Unclosed session element"));
    }
    root.ok_or_else(|| invalid("Missing session root"))
}

impl Session {
    pub fn from_xml(bytes: &[u8]) -> io::Result<Self> {
        let root = parse_xml(bytes)?;
        if root.name != "NotepadPlus" {
            return Err(invalid("Expected NotepadPlus root"));
        }
        let mut sessions = root
            .children
            .into_iter()
            .filter(|node| node.name == "Session");
        let node = sessions
            .next()
            .ok_or_else(|| invalid("Missing Session element"))?;
        if sessions.next().is_some() {
            return Err(invalid("Multiple Session elements"));
        }
        let mut session = Self {
            active_view: number(&node, "activeView", 0),
            ..Self::default()
        };
        let mut seen = [false; 2];
        for child in node.children {
            let index = match child.name.as_str() {
                "mainView" => 0,
                "subView" => 1,
                _ => {
                    session.extra_nodes.push(child);
                    continue;
                }
            };
            if std::mem::replace(&mut seen[index], true) {
                return Err(invalid("Duplicate session view"));
            }
            let mut view = SessionView {
                active_index: number(&child, "activeIndex", 0),
                files: Vec::new(),
            };
            for file in child.children {
                if file.name != "File" {
                    continue;
                }
                if file.attributes.get("filename").is_none_or(String::is_empty) {
                    return Err(invalid("Missing session filename"));
                }
                let mut entry = SessionFile {
                    attributes: file.attributes,
                    ..SessionFile::default()
                };
                for item in file.children {
                    match item.name.as_str() {
                        "Mark" | "Fold" => {
                            if let Some(line) = item
                                .attributes
                                .get("line")
                                .and_then(|value| value.parse::<i64>().ok())
                                .and_then(|value| usize::try_from(value).ok())
                            {
                                if item.name == "Mark" {
                                    entry.bookmarks.push(line);
                                } else {
                                    entry.folds.push(line);
                                }
                            }
                        }
                        _ => entry.extra_nodes.push(item),
                    }
                }
                view.files.push(entry);
            }
            if index == 0 {
                session.main_view = view;
            } else {
                session.sub_view = view;
            }
        }
        Ok(session)
    }

    pub fn to_xml(&self) -> io::Result<Vec<u8>> {
        let mut session = SessionNode {
            name: "Session".into(),
            attributes: BTreeMap::from([("activeView".into(), self.active_view.to_string())]),
            children: Vec::new(),
        };
        for (name, view) in [("mainView", &self.main_view), ("subView", &self.sub_view)] {
            let mut node = SessionNode {
                name: name.into(),
                attributes: BTreeMap::from([("activeIndex".into(), view.active_index.to_string())]),
                children: Vec::new(),
            };
            for file in &view.files {
                if file.filename().is_none_or(str::is_empty) {
                    return Err(invalid("Missing session filename"));
                }
                let mut entry = SessionNode {
                    name: "File".into(),
                    attributes: file.attributes.clone(),
                    children: Vec::new(),
                };
                for (kind, lines) in [("Mark", &file.bookmarks), ("Fold", &file.folds)] {
                    entry.children.extend(lines.iter().map(|line| SessionNode {
                        name: kind.into(),
                        attributes: BTreeMap::from([("line".into(), line.to_string())]),
                        children: Vec::new(),
                    }));
                }
                entry.children.extend(file.extra_nodes.clone());
                node.children.push(entry);
            }
            session.children.push(node);
        }
        session.children.extend(self.extra_nodes.clone());
        let root = SessionNode {
            name: "NotepadPlus".into(),
            children: vec![session],
            ..SessionNode::default()
        };
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
        write_node(&mut writer, &root, 0)?;
        Ok(writer.into_inner())
    }
}

fn write_node(writer: &mut Writer<Vec<u8>>, node: &SessionNode, depth: usize) -> io::Result<()> {
    if depth >= 64 {
        return Err(invalid("Session XML nesting is too deep"));
    }
    let mut start = BytesStart::new(&node.name);
    for (key, value) in &node.attributes {
        start.push_attribute((key.as_str(), value.as_str()));
    }
    if node.children.is_empty() {
        writer.write_event(Event::Empty(start))?;
    } else {
        writer.write_event(Event::Start(start))?;
        for child in &node.children {
            write_node(writer, child, depth + 1)?;
        }
        writer.write_event(Event::End(BytesEnd::new(&node.name)))?;
    }
    Ok(())
}
