use npp_editor_core::{Document, SearchDirection, SearchOptions, Selection};

#[test]
fn ordinary_backward_search_finds_overlapping_matches() {
    let mut doc = Document::from_bytes(b"ababa").unwrap();
    let options = SearchOptions {
        direction: SearchDirection::Backward,
        ..SearchOptions::default()
    };
    for selection in [
        Selection {
            anchor: 2,
            caret: 5,
        },
        Selection {
            anchor: 5,
            caret: 2,
        },
    ] {
        doc.select(selection).unwrap();
        assert_eq!(doc.search_start(options.direction, false), 4);
        assert_eq!(
            doc.search("aba", doc.search_start(options.direction, false), options)
                .unwrap(),
            Some(0..3)
        );
        assert_eq!(
            doc.search("aba", doc.search_start(options.direction, true), options)
                .unwrap(),
            Some(2..5)
        );
    }
}

#[test]
fn backward_origin_rounds_down_to_a_complete_utf8_character() {
    let mut doc = Document::from_bytes("🦀🦀🦀".as_bytes()).unwrap();
    let options = SearchOptions {
        direction: SearchDirection::Backward,
        ..SearchOptions::default()
    };
    doc.select(Selection {
        anchor: 4,
        caret: 12,
    })
    .unwrap();
    assert_eq!(doc.search_start(options.direction, false), 8);
    assert_eq!(doc.search("🦀🦀", 8, options).unwrap(), Some(0..8));
    doc.select(Selection::default()).unwrap();
    assert_eq!(doc.search_start(options.direction, false), 0);
    assert_eq!(doc.search("🦀🦀", 0, options).unwrap(), Some(4..12));
}

#[test]
fn forward_origins_use_the_selected_range_edges() {
    let mut doc = Document::from_bytes(b"ababa").unwrap();
    doc.select(Selection {
        anchor: 5,
        caret: 2,
    })
    .unwrap();
    assert_eq!(doc.search_start(SearchDirection::Forward, false), 5);
    assert_eq!(doc.search_start(SearchDirection::Forward, true), 2);
}
