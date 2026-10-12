//! Portable editor state. Positions are UTF-8 byte offsets at character boundaries.
#![forbid(unsafe_code)]

mod bookmarks;
mod case_fold;
mod decimal_sort;
mod disk;
mod document;
mod editing;
mod encoding;
pub mod find_files;
mod integer_sort;
mod random_sort;
mod search;
mod selection_gesture;
mod session;
mod summary;
mod word_classes;
mod word_parts;
mod workspace;

pub use disk::DiskFingerprint;
pub use document::{Document, EditError, Selection};
pub use editing::{
    CaseChange, DuplicateLines, IndentationStyle, LineOrder, TrimMode, WhitespaceConversion,
};
pub use encoding::{DecodeError, EncodeError, Encoding, LineEnding};
pub use search::{SearchDirection, SearchMode, SearchOptions};
pub use selection_gesture::SelectionGesture;
pub use session::{Session, SessionFile, SessionNode, SessionView};
pub use summary::DocumentSummary;
pub use workspace::{BufferId, CloseError, CloseScope, RecentFile, Workspace};
