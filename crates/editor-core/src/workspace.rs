use crate::{Document, Encoding};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseError {
    UnknownBuffer,
    UnsavedChanges,
}

#[derive(Clone, Copy, Debug)]
pub enum CloseScope {
    All,
    Others,
    Left,
    Right,
    Unchanged,
}

/// Stable buffer identities survive tab reordering and closing adjacent tabs.
#[derive(Default)]
pub struct Workspace {
    buffers: Vec<(BufferId, Document)>,
    active: Option<BufferId>,
    next_id: u64,
    recently_closed: VecDeque<RecentFile>,
    untitled_names: HashMap<BufferId, (usize, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentFile {
    pub path: PathBuf,
    pub encoding: Encoding,
}

impl Workspace {
    /// File names or paths in tab order, including untitled buffers and a trailing
    /// CRLF per entry, as used by Notepad++'s Copy All File Names/Paths commands.
    /// OsString retains native path characters without lossy Unicode conversion.
    pub fn file_list(&self, full_paths: bool) -> std::ffi::OsString {
        let mut output = std::ffi::OsString::new();
        for (id, document) in &self.buffers {
            if let Some(path) = document.path() {
                output.push(if full_paths {
                    path.as_os_str()
                } else {
                    path.file_name().unwrap_or(path.as_os_str())
                });
            } else if let Some(name) = self.untitled_name(*id) {
                output.push(name);
            }
            if !output.is_empty() && !output.as_encoded_bytes().ends_with(b"\r\n") {
                output.push("\r\n");
            }
        }
        output
    }

    /// An untitled name belongs to a buffer, not its current tab position.
    /// Saving releases its number for the next new document, as upstream does.
    pub fn untitled_name(&self, id: BufferId) -> Option<&str> {
        if self.get(id)?.path().is_some() {
            return None;
        }
        self.untitled_names.get(&id).map(|(_, name)| name.as_str())
    }
    pub fn latest_closed_path(&self) -> Option<&Path> {
        self.recently_closed
            .front()
            .map(|entry| entry.path.as_path())
    }
    pub fn latest_closed(&self) -> Option<&RecentFile> {
        self.recently_closed.front()
    }
    pub fn forget_recent_path(&mut self, path: &Path) {
        self.recently_closed.retain(|entry| entry.path != path);
    }
    /// Cycle through visible tab order, wrapping at either end.
    pub fn activate_adjacent(&mut self, forward: bool) -> Option<BufferId> {
        let index = self
            .buffers
            .iter()
            .position(|(id, _)| Some(*id) == self.active)?;
        let next = if forward {
            (index + 1) % self.buffers.len()
        } else if index == 0 {
            self.buffers.len() - 1
        } else {
            index - 1
        };
        let id = self.buffers[next].0;
        self.active = Some(id);
        Some(id)
    }
    /// Moving a tab retains its identity and does not wrap.
    pub fn move_active(&mut self, forward: bool) -> bool {
        let Some(index) = self
            .buffers
            .iter()
            .position(|(id, _)| Some(*id) == self.active)
        else {
            return false;
        };
        let next = if forward {
            if index + 1 >= self.buffers.len() {
                return false;
            }
            index + 1
        } else {
            if index == 0 {
                return false;
            }
            index - 1
        };
        self.buffers.swap(index, next);
        true
    }
    pub fn close_targets(&self, scope: CloseScope) -> Vec<BufferId> {
        let active_index = self
            .buffers
            .iter()
            .position(|(id, _)| Some(*id) == self.active);
        self.buffers
            .iter()
            .enumerate()
            .filter(|(index, (id, doc))| match scope {
                CloseScope::All => true,
                CloseScope::Others => Some(*id) != self.active,
                CloseScope::Left => active_index.is_some_and(|active| *index < active),
                CloseScope::Right => active_index.is_some_and(|active| *index > active),
                CloseScope::Unchanged => !doc.is_dirty(),
            })
            .map(|(_, (id, _))| *id)
            .collect()
    }
    pub fn add(&mut self, document: Document) -> BufferId {
        if let Some(path) = document.path() {
            self.forget_recent_path(path);
        }
        let id = BufferId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("buffer identifiers exhausted");
        if document.path().is_none() {
            let used: HashSet<_> = self
                .buffers
                .iter()
                .filter(|(_, document)| document.path().is_none())
                .filter_map(|(id, _)| self.untitled_names.get(id).map(|(number, _)| *number))
                .collect();
            let number = (1..=used.len() + 1)
                .find(|number| !used.contains(number))
                .unwrap();
            self.untitled_names
                .insert(id, (number, format!("new {number}")));
        }
        self.buffers.push((id, document));
        self.active = Some(id);
        id
    }
    pub fn active(&self) -> Option<BufferId> {
        self.active
    }
    pub fn buffers(&self) -> impl Iterator<Item = (BufferId, &Document)> {
        self.buffers.iter().map(|(id, doc)| (*id, doc))
    }
    pub fn get(&self, id: BufferId) -> Option<&Document> {
        self.buffers
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, doc)| doc)
    }
    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut Document> {
        self.buffers
            .iter_mut()
            .find(|(key, _)| *key == id)
            .map(|(_, doc)| doc)
    }
    pub fn activate(&mut self, id: BufferId) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.active = Some(id);
        true
    }
    pub fn close(&mut self, id: BufferId, discard_changes: bool) -> Result<Document, CloseError> {
        let index = self
            .buffers
            .iter()
            .position(|(key, _)| *key == id)
            .ok_or(CloseError::UnknownBuffer)?;
        if self.buffers[index].1.is_dirty() && !discard_changes {
            return Err(CloseError::UnsavedChanges);
        }
        let (_, document) = self.buffers.remove(index);
        self.untitled_names.remove(&id);
        if let Some(path) = document.path() {
            self.forget_recent_path(path);
            self.recently_closed.push_front(RecentFile {
                path: path.to_owned(),
                encoding: document.saved_encoding(),
            });
            self.recently_closed.truncate(10);
        }
        if self.active == Some(id) {
            self.active = self
                .buffers
                .get(index)
                .or_else(|| self.buffers.last())
                .map(|(key, _)| *key);
        }
        Ok(document)
    }
}
