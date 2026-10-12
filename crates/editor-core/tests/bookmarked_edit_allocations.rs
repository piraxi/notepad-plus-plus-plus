use npp_editor_core::Document;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct MeasuredAllocator;
static TRACKING: AtomicBool = AtomicBool::new(false);
static LARGEST: AtomicUsize = AtomicUsize::new(0);

fn record(bytes: usize) {
    if TRACKING.load(Ordering::Relaxed) {
        LARGEST.fetch_max(bytes, Ordering::Relaxed);
    }
}

// SAFETY: All allocations and deallocations are forwarded unchanged to System;
// the extra bookkeeping uses only atomics and never allocates recursively.
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

#[test]
fn small_edit_undo_and_redo_do_not_copy_a_large_bookmarked_document() {
    let text = "a".repeat(2 * 1024 * 1024);
    let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
    doc.toggle_bookmark(0).unwrap();
    let middle = text.len() / 2;
    LARGEST.store(0, Ordering::Relaxed);
    TRACKING.store(true, Ordering::Relaxed);
    doc.replace(middle..middle + 1, "b").unwrap();
    doc.undo().unwrap();
    doc.redo().unwrap();
    TRACKING.store(false, Ordering::Relaxed);
    // Allow small history/index allocations without tying this test to their
    // exact representation. A document-sized shadow copy would fail this bound.
    assert!(LARGEST.load(Ordering::Relaxed) < 64 * 1024);
    assert_eq!(doc.history_text_bytes(), 1);
    assert_eq!(doc.bookmarked_lines().collect::<Vec<_>>(), vec![0]);
    assert_eq!(&doc.text()[middle..middle + 1], "b");
}
