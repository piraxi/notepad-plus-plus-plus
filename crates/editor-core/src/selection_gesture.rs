use crate::{Document, EditError, Selection};
use std::sync::{
    Arc,
    atomic::{AtomicIsize, Ordering},
};

/// A view's Begin/End Select position, shared with documents edited in that view.
#[derive(Clone)]
pub struct SelectionGesture(Arc<AtomicIsize>);

impl Default for SelectionGesture {
    fn default() -> Self {
        Self(Arc::new(AtomicIsize::new(-1)))
    }
}

impl SelectionGesture {
    pub fn is_started(&self) -> bool {
        self.0.load(Ordering::Relaxed) != -1
    }

    pub fn toggle(&self, document: &mut Document) -> Result<(), EditError> {
        let caret = document.selection().caret;
        let start = self.0.load(Ordering::Relaxed);
        if start == -1 {
            self.0.store(caret as isize, Ordering::Relaxed);
        } else {
            // SCI_SETSEL treats a negative anchor as the current caret.
            let mut anchor = if start < 0 {
                caret
            } else {
                (start as usize).min(document.text().len())
            };
            while !document.text().is_char_boundary(anchor) {
                anchor += 1;
            }
            document.select(Selection { anchor, caret })?;
            self.0.store(-1, Ordering::Relaxed);
        }
        Ok(())
    }

    pub(crate) fn replaced(&self, position: usize, removed: usize, inserted: usize) {
        // Source updateBeginEndSelectPosition handles deletion then insertion,
        // testing the updated position each time, and excludes the byte just
        // before the marker as well as the marker itself.
        for (length, insertion) in [(removed, false), (inserted, true)] {
            let start = self.0.load(Ordering::Relaxed);
            if length != 0 && start != -1 && (position as isize) < start.wrapping_sub(1) {
                let next = if insertion {
                    start.wrapping_add(length as isize)
                } else {
                    start.wrapping_sub(length as isize)
                };
                self.0.store(next, Ordering::Relaxed);
            }
        }
    }
}
