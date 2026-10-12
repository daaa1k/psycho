use psycho::PresentationDocument;

#[test]
fn insertion_boundaries_preserve_source_and_history() {
    let original = "presentation {\n\tmetadata { title \"title\" }\n\t// keep this\n\tslide id=\"first\" { text #\"raw \\\"text\"#; } // trailing\n\tslide id=\"last\" {}\n}\n";
    for at in 0..=2 {
        let mut document = PresentationDocument::from_source(original).unwrap();
        assert_eq!(document.insert_slide(at).unwrap(), at);
        let slides = &document.model().unwrap().slides;
        assert_eq!(slides.len(), 3);
        assert_eq!(slides[at].id, None);
        assert!(document.source().contains("\t// keep this\n"));
        assert!(
            document
                .source()
                .contains("\tslide id=\"first\" { text #\"raw \\\"text\"#; } // trailing\n")
        );
        let after = document.source().to_owned();
        assert!(document.undo());
        assert_eq!(document.source(), original);
        assert!(document.redo());
        assert_eq!(document.source(), after);
    }
    let mut document = PresentationDocument::from_source(original).unwrap();
    assert!(document.insert_slide(3).is_err());
    assert_eq!(document.source(), original);
    assert!(!document.can_undo());
    document.move_slide(0, 0).unwrap();
    assert_eq!(document.source(), original);
    assert!(!document.can_undo());
    assert!(document.move_slide(0, 2).is_err());
    assert!(!document.can_undo());
}

#[test]
fn empty_presentation_can_insert_and_remove_without_removing_assets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let asset = dir.path().join("asset.png");
    std::fs::write(&asset, b"asset bytes").unwrap();
    let original = "presentation { metadata { title \"empty\"; }; }";
    std::fs::write(&path, original).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    assert_eq!(document.insert_slide(0).unwrap(), 0);
    document.remove_slide(0).unwrap();
    assert!(document.model().unwrap().slides.is_empty());
    assert_eq!(std::fs::read(asset).unwrap(), b"asset bytes");
    assert!(document.undo());
    assert_eq!(document.model().unwrap().slides.len(), 1);
    assert!(document.undo());
    assert_eq!(document.source(), original);
}

#[test]
fn insertion_and_move_preserve_unique_ids_and_inline_siblings() {
    let original =
        "presentation { metadata { title \"inline\"; }; slide id=\"a\" {}; slide id=\"b\" {}; }";
    let mut document = PresentationDocument::from_source(original).unwrap();
    document.insert_slide(1).unwrap();
    document.move_slide(2, 0).unwrap();
    let slides = &document.model().unwrap().slides;
    assert_eq!(
        slides
            .iter()
            .map(|slide| slide.id.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("b"), Some("a"), None]
    );
    assert!(document.undo());
    assert!(document.undo());
    assert_eq!(document.source(), original);
}
