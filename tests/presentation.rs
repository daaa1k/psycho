use std::fs;

use psycho::{AssetProblem, PresentationDocument};

const VALID: &str = "presentation {\n    metadata { title \"Builds\" }\n    slide id=\"intro\" { heading \"Before\" }\n}\n";

#[test]
fn opening_and_read_only_save_preserve_every_byte() {
    let input = "\u{feff}// presentation\r\npresentation {\r\n    metadata { title #\"Builds\"# }\r\n    slide { text \"same\" } \r\n}\r\n";
    let document = PresentationDocument::from_source(input).unwrap();

    assert_eq!(document.source(), input);
    assert!(!document.is_dirty());
    assert_eq!(document.model().unwrap().title, "Builds");
}

#[test]
fn schema_diagnostics_reject_unknown_nodes_and_duplicate_attributes() {
    let input = "presentation {\n metadata { title \"x\" }\n slide id=\"one\" id=\"two\" {\n  mystery\n }\n}";
    let document = PresentationDocument::from_source(input).unwrap();

    let messages = document
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages.iter().any(|message| message.contains("duplicate")),
        "{messages:?}"
    );
    assert!(messages.iter().any(|message| message.contains("unknown")));
    assert!(!document.can_edit());
}

#[test]
fn changing_a_value_replaces_only_its_source_token_and_undo_restores_bytes() {
    let input = "// keep\r\npresentation {\r\n    metadata { title #\"Builds\"# } // tail\r\n    slide { heading \"Before\" }\r\n}\r\n";
    let mut document = PresentationDocument::from_source(input).unwrap();

    document.set_title("A new presentation").unwrap();
    let edited = input.replace("#\"Builds\"#", "\"A new presentation\"");
    assert_eq!(document.source(), edited);
    assert!(document.is_dirty());
    assert!(document.undo());
    assert_eq!(document.source(), input);
    assert!(document.redo());
    assert_eq!(document.source(), edited);
}

#[test]
fn invalid_kdl_is_reported_with_a_source_location() {
    let document =
        PresentationDocument::from_source("presentation { metadata { title \"unterminated }\n")
            .unwrap();

    assert!(!document.diagnostics().is_empty());
    assert!(document.diagnostics()[0].line >= 1);
    assert!(!document.can_edit());
}

#[test]
fn save_refuses_to_overwrite_an_external_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    fs::write(&path, VALID).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("edited locally").unwrap();
    fs::write(&path, VALID.replace("Builds", "edited elsewhere")).unwrap();

    let error = document.save().unwrap_err();
    assert!(error.to_string().contains("external"));
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        VALID.replace("Builds", "edited elsewhere")
    );
    assert!(document.is_dirty());
}

#[test]
fn save_writes_only_the_requested_change_and_marks_the_document_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    fs::write(&path, VALID).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("saved").unwrap();

    document.save().unwrap();

    assert_eq!(
        fs::read_to_string(path).unwrap(),
        VALID.replace("Builds", "saved")
    );
    assert!(!document.is_dirty());
}

#[test]
fn save_as_does_not_overwrite_an_existing_destination() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.kdl");
    fs::write(&path, "keep me").unwrap();
    let mut document = PresentationDocument::from_source(VALID).unwrap();

    assert!(document.save_as(&path).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "keep me");
}

#[test]
fn adding_elements_to_a_blank_slide_and_undoing_keeps_valid_kdl() {
    let input = "presentation {\n    metadata { title \"Blank\" }\n    slide {}\n}\n";
    let mut document = PresentationDocument::from_source(input).unwrap();

    let index = document
        .add_element(0, psycho::ElementKind::Heading)
        .unwrap();
    assert_eq!(index, 0);
    assert_eq!(
        document.model().unwrap().slides[0].elements,
        vec![psycho::Element::Heading(String::new())]
    );
    assert!(document.source().contains("heading \"\""));
    assert!(document.undo());
    assert_eq!(document.source(), input);
}

#[test]
fn adding_and_removing_slides_preserves_the_metadata_and_previous_slides() {
    let mut document = PresentationDocument::from_source(VALID).unwrap();
    let index = document.add_slide().unwrap();

    assert_eq!(index, 1);
    assert_eq!(document.model().unwrap().slides.len(), 2);
    document.remove_slide(0).unwrap();
    assert_eq!(document.model().unwrap().slides.len(), 1);
    assert_eq!(document.model().unwrap().slides[0].id.as_deref(), None);
}

#[test]
fn bullet_items_can_be_changed_without_rewriting_other_slide_text() {
    let input = "presentation {\n metadata { title \"Lists\" }\n slide {\n  bullets {\n   item #\"one\"# // keep tail\n   item \"two\"\n  }\n  text \"outside\"\n }\n}";
    let mut document = PresentationDocument::from_source(input).unwrap();
    assert!(document.can_edit(), "{:?}", document.diagnostics());

    document
        .set_bullets_text(0, 0, "first\nsecond\nthird")
        .unwrap();
    assert!(document.source().contains("item \"first\" // keep tail"));
    assert!(document.source().contains("text \"outside\""));
    assert_eq!(
        document.model().unwrap().slides[0].elements[0],
        psycho::Element::Bullets(vec!["first".into(), "second".into(), "third".into()])
    );
    assert!(document.undo());
    assert_eq!(document.source(), input);
}

#[test]
fn column_widths_change_together_and_invalid_totals_are_rejected() {
    let input = "presentation {\n metadata { title \"Columns\" }\n slide { columns { column width=50; column width=50 } }\n}";
    let mut document = PresentationDocument::from_source(input).unwrap();

    assert!(document.set_column_widths(0, 0, 40, 60).is_ok());
    assert!(document.source().contains("width=40"));
    assert!(document.source().contains("width=60"));
    let edited = document.source().to_owned();
    assert!(document.set_column_widths(0, 0, 40, 50).is_err());
    assert_eq!(document.source(), edited);
}

#[test]
fn bundled_presentation_parses_and_its_png_asset_is_available() {
    let document = PresentationDocument::from_source_with_asset_base(
        include_str!("../examples/build-time.kdl"),
        env!("CARGO_MANIFEST_DIR"),
    )
    .unwrap();

    assert!(document.can_edit(), "{:?}", document.diagnostics());
    assert_eq!(document.model().unwrap().slides.len(), 6);
    assert!(document.inspect_assets().is_empty());
}

#[test]
fn image_assets_resolve_relative_to_the_kdl_file_and_report_missing_or_corrupt_files() {
    let dir = tempfile::tempdir().unwrap();
    let image_path = dir.path().join("chart.data");
    let document_path = dir.path().join("presentation.kdl");
    fs::write(
        &image_path,
        include_bytes!("../assets/build-time.png").as_slice(),
    )
    .unwrap();
    fs::write(
        &document_path,
        "presentation {\n    metadata { title \"Assets\" }\n    slide { image \"chart.data\" }\n}\n",
    )
    .unwrap();
    let document = PresentationDocument::open(&document_path).unwrap();

    assert!(document.can_edit(), "{:?}", document.diagnostics());
    assert_eq!(document.model().unwrap().slides[0].elements.len(), 1);
    assert!(document.inspect_assets().is_empty());

    fs::remove_file(&image_path).unwrap();
    let missing = document.inspect_assets();
    assert_eq!(missing[0].problem, AssetProblem::Missing);

    fs::write(&image_path, b"\x89PNG\r\n\x1a\nnot a valid png").unwrap();
    let corrupt = document.inspect_assets();
    assert_eq!(corrupt[0].problem, AssetProblem::Corrupt);
}

#[test]
fn moving_a_slide_preserves_source_and_keeps_inline_comments_with_it() {
    let input = "presentation {\n    metadata { title \"Move\" }\n    // stays before the second slide\n    slide id=\"one\" { heading \"One\" } // follows one\n    slide id=\"two\" { heading \"Two\" }\n    slide id=\"three\" { heading \"Three\" }\n}\n";
    let mut document = PresentationDocument::from_source(input).unwrap();

    document.move_slide(0, 2).unwrap();

    let model = document.model().unwrap();
    assert_eq!(model.slides[0].id.as_deref(), Some("two"));
    assert_eq!(model.slides[1].id.as_deref(), Some("three"));
    assert_eq!(model.slides[2].id.as_deref(), Some("one"));
    assert!(
        document
            .source()
            .contains("// stays before the second slide\n    slide id=\"two\"")
    );
    assert!(
        document
            .source()
            .contains("slide id=\"one\" { heading \"One\" } // follows one")
    );
    assert!(document.undo());
    assert_eq!(document.source(), input);
}

#[test]
fn moving_a_slide_rejects_multiple_nodes_on_one_source_line() {
    let input =
        "presentation { metadata { title \"Move\" } slide id=\"one\" {}; slide id=\"two\" {} }";
    let mut document = PresentationDocument::from_source(input).unwrap();
    let before = document.source().to_owned();

    assert!(document.move_slide(0, 1).is_err());
    assert_eq!(document.source(), before);
}

#[test]
fn moving_an_element_keeps_trailing_comments_and_undo_restores_source() {
    let input = "presentation {\n    metadata { title \"Move\" }\n    slide {\n        heading \"One\" // follows heading\n        text \"Two\"\n        bullets { item \"Three\" }\n    }\n}\n";
    let mut document = PresentationDocument::from_source(input).unwrap();

    document.move_element(0, 0, 2).unwrap();

    assert_eq!(
        document.model().unwrap().slides[0].elements,
        vec![
            psycho::Element::Text("Two".into()),
            psycho::Element::Bullets(vec!["Three".into()]),
            psycho::Element::Heading("One".into()),
        ]
    );
    assert!(
        document
            .source()
            .contains("heading \"One\" // follows heading")
    );
    assert!(document.undo());
    assert_eq!(document.source(), input);
}
