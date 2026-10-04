use psycho::{DocumentError, Element, ElementField, ElementKind, PresentationDocument};
use std::fs;

const SOURCE: &str = "presentation {\n    metadata { title \"日本語\" }\n    slide {\n        // independent\n        text #\"original\"# // attached\n        columns {\n            column width=50 { text \"left\" }\n            column width=50 { text \"right\" }\n        }\n    }\n}\n";

#[test]
fn schema_constraints_are_reported_together_without_partial_preview() {
    let invalid_elements = [
        "heading",
        "text 1",
        "text \"one\" \"extra\"",
        "heading \"x\" unknown=1",
        "text \"x\" { child; }",
        "bullets \"unexpected\"",
        "bullets { text \"x\"; }",
        "bullets { item 1; }",
        "code \"x\" language=1",
        "code \"x\" language=\"rust\" language=\"yaml\"",
        "image \"\"",
        "image \"/absolute.png\"",
        "image 1",
        "image \"x.png\" { caption 1; }",
        "image \"x.png\" { caption \"one\"; caption \"two\"; }",
        "image \"x.png\" { text \"wrong\"; }",
        "columns",
        "columns { column width=50; }",
        "columns { column width=50; column width=50; column width=50; }",
        "columns { column; column width=50; }",
        "columns { column width=0; column width=100; }",
        "columns { column width=50.0; column width=50; }",
        "columns { column width=40; column width=50; }",
        "columns { column width=50 width=50; column width=50; }",
        "columns { column width=50 { columns; }; column width=50; }",
        "unknown",
        "(typed)text \"x\"",
        "text (typed)\"x\"",
    ];
    for element in invalid_elements {
        let source =
            format!("presentation {{\n metadata {{ title \"test\" }}\n slide {{ {element}; }}\n}}");
        let doc = PresentationDocument::from_source(&source).unwrap();
        assert!(!doc.can_edit(), "{element}");
        assert!(doc.model().is_none(), "{element}");
        assert!(
            doc.diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.kind == psycho::DiagnosticKind::Schema),
            "{element}: {:?}",
            doc.diagnostics()
        );
    }
    let doc = PresentationDocument::from_source(
        "presentation {\n metadata { title \"test\" }\n slide { heading 1; text 2; }\n}",
    )
    .unwrap();
    assert!(doc.diagnostics().len() >= 2);
    for diagnostic in doc.diagnostics() {
        assert_eq!(diagnostic.line, 3);
        assert_eq!(diagnostic.source_line, " slide { heading 1; text 2; }");
        assert!(doc.source().get(diagnostic.byte_range.clone()).is_some());
    }
}

#[test]
fn moving_multiline_and_raw_strings_keeps_their_meaning() {
    for value in [
        "#\"quotes \\\" and // text\"#",
        "\"\"\"\n            日本語\n            second line\n            \"\"\"",
        "#\"\"\"\n            // literal comment\n            \\\\literal tab\n            \"\"\"#",
        "\"line one\\nline two\\tend\"",
    ] {
        let source = format!(
            "presentation {{\n    metadata {{ title \"test\" }}\n    slide {{\n        /* outer /* nested */ comment */\n        /- text \"ignored\"\n        text {value}; // attached\n        columns {{\n            column width=50\n            column width=50\n        }}\n    }}\n}}"
        );
        let mut doc = PresentationDocument::from_source(&source).unwrap();
        assert!(doc.can_edit(), "{value}: {:?}", doc.diagnostics());
        let expected = doc.model().unwrap().slides[0].elements[0].clone();
        doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
        let Element::Columns { left, .. } = &doc.model().unwrap().slides[0].elements[0] else {
            panic!()
        };
        assert_eq!(left, &vec![expected.clone()]);
        doc.move_column_element_to_slide_at(0, 0, 0, 0, 0).unwrap();
        assert_eq!(doc.model().unwrap().slides[0].elements[0], expected);
        assert!(doc.source().contains("/- text \"ignored\""));
        doc.undo();
        doc.undo();
        assert_eq!(doc.source(), source);
    }
}

#[cfg(unix)]
#[test]
fn failed_write_keeps_original_edit_and_history_for_retry() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    fs::write(&path, SOURCE).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    doc.set_title("edited").unwrap();
    let edited = doc.source().to_owned();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
    let failed = doc.save();
    // Restore directory permissions even if the assertion below fails.
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(failed.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), SOURCE);
    assert_eq!(doc.source(), edited);
    assert!(doc.is_dirty() && doc.can_undo());
    doc.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), edited);
    assert!(!doc.is_dirty() && doc.can_undo());
}

#[test]
fn six_slide_example_loads_with_all_assets_from_its_file_location() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/build-time.kdl");
    let doc = PresentationDocument::open(path).unwrap();
    assert_eq!(doc.model().unwrap().slides.len(), 6);
    assert!(doc.inspect_assets().is_empty());
}

#[test]
fn inline_semicolon_nodes_can_reorder_at_every_boundary() {
    let source = "presentation { metadata { title \"test\" }; slide { text \"a\"; text \"b\"; text \"c\"; }; slide { text \"last\"; }; }";
    for from in 0..3 {
        for to in 0..3 {
            let mut doc = PresentationDocument::from_source(source).unwrap();
            let mut expected = doc.model().unwrap().clone();
            let moving = expected.slides[0].elements.remove(from);
            expected.slides[0].elements.insert(to, moving);
            doc.move_element(0, from, to)
                .unwrap_or_else(|error| panic!("{from} -> {to}: {error}; {}", doc.source()));
            assert_eq!(doc.model().unwrap(), &expected);
            if from != to {
                assert!(doc.undo());
            }
            assert_eq!(doc.source(), source);
        }
    }
    let mut doc = PresentationDocument::from_source(source).unwrap();
    let mut expected = doc.model().unwrap().clone();
    expected.slides.swap(0, 1);
    doc.move_slide(0, 1).unwrap();
    assert_eq!(doc.model().unwrap(), &expected);
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
}

#[test]
fn save_preserves_bytes_and_value_edit_is_local() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace('\n', "\r\n"),
        format!("\u{feff}{}", SOURCE.trim_end()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("presentation.kdl");
        fs::write(&path, &source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(doc.can_edit(), "{:?}", doc.diagnostics());
        doc.save().unwrap();
        assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
        doc.set_element_text(0, 0, "変更").unwrap();
        assert_eq!(doc.source(), source.replace("#\"original\"#", "\"変更\""));
        doc.save().unwrap();
        assert!(doc.can_undo());
        assert!(!doc.is_dirty());
        assert!(doc.undo());
        assert_eq!(doc.source(), source);
        assert!(doc.is_dirty());
        assert!(doc.redo());
        assert!(!doc.is_dirty());
    }
}

#[test]
fn move_into_and_out_of_columns_preserves_text_and_comment_ownership() {
    let mut doc = PresentationDocument::from_source(SOURCE).unwrap();
    doc.move_element_to_column_at(0, 0, 1, 1, 0).unwrap();
    let moved = doc.source().to_owned();
    assert!(moved.contains("// independent"));
    assert!(moved.contains("text #\"original\"# // attached"));
    let Element::Columns { right, .. } = &doc.model().unwrap().slides[0].elements[0] else {
        panic!()
    };
    assert_eq!(right[0], Element::Text("original".into()));
    doc.move_column_element_to_slide_at(0, 0, 1, 0, 0).unwrap();
    assert_eq!(
        doc.model().unwrap().slides[0].elements[0],
        Element::Text("original".into())
    );
    assert!(doc.undo());
    assert_eq!(doc.source(), moved);
    assert!(doc.undo());
    assert_eq!(doc.source(), SOURCE);
    doc.remove_element(0, 0).unwrap();
    assert!(doc.source().contains("// independent"));
    assert!(!doc.source().contains("// attached"));
    assert!(doc.undo());
    assert_eq!(doc.source(), SOURCE);
}

#[test]
fn blank_slide_supports_all_elements_and_caption_and_rejects_nested_columns() {
    let mut doc =
        PresentationDocument::from_source("presentation { metadata { title \"test\" } }").unwrap();
    let slide = doc.add_slide().unwrap();
    for kind in [
        ElementKind::Heading,
        ElementKind::Text,
        ElementKind::Bullets,
        ElementKind::Code,
        ElementKind::Image,
        ElementKind::Columns,
    ] {
        doc.add_element(slide, kind).unwrap();
    }
    doc.set_element_field(0, 4, ElementField::Caption, "説明\n2行目")
        .unwrap();
    doc.set_element_field(0, 3, ElementField::CodeLanguage, "rust")
        .unwrap();
    doc.set_bullets_text(0, 2, "一\n二\n三").unwrap();
    doc.set_column_widths(0, 5, 33, 67).unwrap();
    for column in 0..2 {
        doc.add_column_element(0, 5, column, ElementKind::Text)
            .unwrap();
    }
    let before = doc.source().to_owned();
    assert!(
        doc.add_column_element(0, 5, 0, ElementKind::Columns)
            .is_err()
    );
    assert!(doc.move_element_to_column(0, 5, 5, 0).is_err());
    assert_eq!(doc.source(), before);
    doc.remove_element(0, 5).unwrap();
    assert!(doc.undo());
    assert_eq!(doc.source(), before);
    doc.remove_slide(0).unwrap();
    assert!(doc.model().unwrap().slides.is_empty());
    assert!(doc.undo());
    assert_eq!(doc.source(), before);
}

#[test]
fn history_limits_branching_and_same_value_operations() {
    let mut doc = PresentationDocument::from_source(SOURCE).unwrap();
    doc.set_title("first").unwrap();
    doc.set_title("second").unwrap();
    doc.undo();
    doc.set_title("first").unwrap();
    assert!(doc.can_redo());
    doc.set_title("branch").unwrap();
    assert!(!doc.can_redo());
    for i in 0..1001 {
        doc.set_title(&format!("title-{i}")).unwrap();
    }
    let mut count = 0;
    while doc.undo() {
        count += 1;
    }
    assert_eq!(count, 1000);
    assert_eq!(doc.model().unwrap().title, "title-0");
}

#[test]
fn external_change_and_deletion_never_overwrite_or_discard_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    fs::write(&path, SOURCE).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    doc.set_title("edited").unwrap();
    let edited = doc.source().to_owned();
    fs::write(&path, "external").unwrap();
    assert!(matches!(
        doc.check_external_change(),
        Err(DocumentError::ExternalChange)
    ));
    assert!(matches!(doc.save(), Err(DocumentError::ExternalChange)));
    assert_eq!(fs::read_to_string(&path).unwrap(), "external");
    assert_eq!(doc.source(), edited);
    assert!(doc.is_dirty() && doc.can_undo());
    fs::remove_file(&path).unwrap();
    assert!(doc.save().is_err());
    assert!(!path.exists());
    assert_eq!(doc.source(), edited);
    assert!(doc.is_dirty() && doc.can_undo());
}

#[test]
fn failed_retreat_preserves_state_and_success_rebases_assets_and_clears_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    fs::write(&path, SOURCE).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    doc.add_element(0, ElementKind::Image).unwrap();
    let edited = doc.source().to_owned();
    assert!(doc.save_as(dir.path().join("missing/retreat.kdl")).is_err());
    assert_eq!(doc.path(), Some(path.as_path()));
    assert_eq!(doc.source(), edited);
    assert!(doc.is_dirty() && doc.can_undo());
    assert!(matches!(
        doc.save_as(&path),
        Err(DocumentError::DestinationExists)
    ));
    fs::create_dir(dir.path().join("retreat")).unwrap();
    let target = dir.path().join("retreat/presentation.kdl");
    doc.save_as(&target).unwrap();
    assert_eq!(doc.path(), Some(target.as_path()));
    assert!(!doc.is_dirty() && !doc.can_undo());
    assert_eq!(
        doc.model().unwrap().slides[0].image_paths(),
        vec!["../assets/image.png"]
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), SOURCE);
    assert_eq!(
        PresentationDocument::open(&target).unwrap().source(),
        doc.source()
    );
}

#[test]
fn invalid_documents_are_diagnosed_and_cannot_edit_or_save() {
    for source in [
        "presentation {",
        "presentation { metadata { title \"\" } }",
        "presentation { metadata { title \"test\" } slide { column width=50 } }",
        "presentation { metadata { title \"test\" } slide id=\"same\"; slide id=\"same\" }",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.kdl");
        fs::write(&path, source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(!doc.can_edit());
        assert!(doc.model().is_none());
        assert!(!doc.diagnostics().is_empty());
        assert!(doc.set_title("new").is_err());
        assert!(doc.save().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
fn image_formats_are_detected_by_content_and_missing_images_allow_editing() {
    let dir = tempfile::tempdir().unwrap();
    let source = "presentation {\n metadata { title \"test\" }\n slide { image \"asset.unknown\" { caption \"keep\" } }\n}";
    let mut doc = PresentationDocument::from_source_with_asset_base(source, dir.path()).unwrap();
    assert_eq!(doc.inspect_assets().len(), 1);
    assert!(doc.can_edit());
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
    ] {
        image::RgbImage::from_pixel(2, 2, image::Rgb([100, 150, 200]))
            .save_with_format(dir.path().join("asset.unknown"), format)
            .unwrap();
        assert!(doc.inspect_assets().is_empty(), "{format:?}");
    }
    fs::write(dir.path().join("asset.unknown"), b"broken").unwrap();
    assert_eq!(doc.inspect_assets().len(), 1);
    doc.set_element_field(0, 0, ElementField::Caption, "edited")
        .unwrap();
    assert!(doc.can_undo());
    doc.inspect_assets();
    assert!(doc.can_undo());
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
}

#[test]
fn valid_kdl_whitespace_and_continuations_survive_local_edits_and_moves() {
    for newline in [
        "\n", "\r\n", "\r", "\u{85}", "\u{2028}", "\u{2029}", "\u{c}",
    ] {
        for space in [" ", "\t", "\u{a0}", "\u{2003}"] {
            let source = format!(
                "presentation {{{newline}{space}metadata {{ title \"test\" }}{newline}{space}slide {{{newline}{space}{space}text \\\n{space}{space}\"first\"{newline}{space}{space}text \"second\"{newline}{space}}}{newline}}}"
            );
            let mut doc = PresentationDocument::from_source(&source).unwrap();
            assert!(
                doc.can_edit(),
                "{newline:?} {space:?}: {:?}",
                doc.diagnostics()
            );
            doc.set_element_text(0, 0, "日本語").unwrap();
            assert_eq!(doc.source(), source.replace("\"first\"", "\"日本語\""));
            doc.undo();
            let mut expected = doc.model().unwrap().clone();
            expected.slides[0].elements.swap(0, 1);
            doc.move_element(0, 0, 1)
                .unwrap_or_else(|error| panic!("{newline:?} {space:?}: {error}"));
            assert_eq!(doc.model().unwrap(), &expected);
            doc.undo();
            assert_eq!(doc.source(), source);
        }
    }
}

#[test]
fn multiline_trailing_comments_move_with_the_element() {
    let source = "presentation {\n metadata { title \"test\" }\n slide {\n  text \"first\" /* attached\n    /* nested */ comment */\n  // independent\n  text \"second\"\n }\n}";
    let mut doc = PresentationDocument::from_source(source).unwrap();
    doc.move_element(0, 0, 1).unwrap();
    assert!(
        doc.source()
            .contains("text \"first\" /* attached\n    /* nested */ comment */")
    );
    assert!(
        doc.source().find("// independent").unwrap()
            < doc.source().find("text \"second\"").unwrap()
    );
    doc.undo();
    assert_eq!(doc.source(), source);
}

#[test]
fn inline_bullets_reorder_at_every_boundary_without_moving_parent_content() {
    let source = "\npresentation { metadata { title \"test\" }; slide { bullets { item \"a\"; item \"b\"; item \"c\"; }; text \"after\"; }; }";
    for from in 0..3 {
        for to in 0..3 {
            let mut doc = PresentationDocument::from_source(source).unwrap();
            let mut expected = doc.model().unwrap().clone();
            let Element::Bullets(items) = &mut expected.slides[0].elements[0] else {
                panic!()
            };
            let moving = items.remove(from);
            items.insert(to, moving);
            doc.move_bullet(0, 0, from, to)
                .unwrap_or_else(|error| panic!("{from} -> {to}: {error}"));
            assert_eq!(doc.model().unwrap(), &expected);
            if from != to {
                doc.undo();
            }
            assert_eq!(doc.source(), source);
        }
    }
}

#[test]
fn asset_diagnostics_cover_animation_permissions_and_multiple_locations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("asset.bin");
    let source = "presentation { metadata { title \"test\" }; slide id=\"images\" { image \"asset.bin\"; columns { column width=50 { image \"asset.bin\"; }; column width=50; }; }; }";
    let mut doc = PresentationDocument::from_source_with_asset_base(source, dir.path()).unwrap();
    doc.set_title("edited").unwrap();
    for bytes in [
        include_bytes!("fixtures/animated.png").as_slice(),
        include_bytes!("fixtures/animated.webp").as_slice(),
    ] {
        fs::write(&path, bytes).unwrap();
        let diagnostics = doc.inspect_assets();
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|d| d.problem == psycho::AssetProblem::AnimatedImage)
        );
        assert!(doc.can_edit() && doc.can_undo());
    }
    fs::write(&path, b"GIF89a\x03\0\x02\0").unwrap();
    assert!(
        doc.inspect_assets()
            .iter()
            .all(|d| d.problem == psycho::AssetProblem::UnsupportedFormat)
    );
    image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 0, 0, 0]))
        .save_with_format(&path, image::ImageFormat::Png)
        .unwrap();
    assert!(doc.inspect_assets().is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let diagnostics = doc.inspect_assets();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|d| d.problem == psycho::AssetProblem::PermissionDenied)
        );
    }
    fs::write(&path, include_bytes!("fixtures/orientation-6.jpg")).unwrap();
    assert!(doc.inspect_assets().is_empty());
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
}

#[test]
fn nested_bullet_items_preserve_siblings_and_exact_history() {
    let source = "presentation { metadata { title \"Nested items\" }; slide { text \"Root untouched\"; columns { column width=50 { bullets { item \"a\"; item \"b\"; }; text \"Left untouched\"; }; column width=50 { text \"Right untouched\"; }; }; }; }\r\n";
    let mut doc = PresentationDocument::from_source(source).unwrap();
    let original = doc.model().unwrap().clone();
    doc.add_column_bullet(0, 1, 0, 0, 2, "c").unwrap();
    let added = doc.source().to_owned();
    assert!(added.contains("item \"c\""));
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
    assert!(doc.redo());
    assert_eq!(doc.source(), added);
    doc.move_column_bullet(0, 1, 0, 0, 2, 0).unwrap();
    let moved = doc.source().to_owned();
    assert!(moved.find("item \"c\"") < moved.find("item \"a\""));
    assert!(doc.undo());
    assert_eq!(doc.source(), added);
    assert!(doc.redo());
    assert_eq!(doc.source(), moved);
    doc.remove_column_bullet(0, 1, 0, 0, 0).unwrap();
    assert_eq!(doc.model().unwrap(), &original);
    for sibling in ["Root untouched", "Left untouched", "Right untouched"] {
        assert!(doc.source().contains(sibling));
    }
    assert!(doc.undo());
    assert_eq!(doc.source(), moved);
}

#[test]
fn same_decoded_values_preserve_raw_quotes_empty_bullets_and_redo() {
    let source = "presentation { metadata { title #\"Raw title\"# }; slide { heading #\"Raw heading\"#; bullets { item #\"a\"#; item \"b\"; }; bullets {}; columns { column width=50 { bullets { item #\"c\"#; item \"d\"; }; }; column width=50 {}; }; }; }\r\n";
    let mut doc = PresentationDocument::from_source(source).unwrap();
    doc.set_title("changed").unwrap();
    assert!(doc.undo());
    assert!(doc.can_redo());
    doc.set_title("Raw title").unwrap();
    doc.set_element_text(0, 0, "Raw heading").unwrap();
    doc.set_bullets_text(0, 1, "a\nb").unwrap();
    doc.set_bullets_text(0, 2, "").unwrap();
    doc.set_column_bullets_text(0, 3, 0, 0, "c\nd").unwrap();
    assert_eq!(doc.source(), source);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    doc.set_bullets_text(0, 1, "a\nB").unwrap();
    assert_eq!(doc.source(), source.replace("item \"b\"", "item \"B\""));
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
    doc.set_column_bullets_text(0, 3, 0, 0, "c\nD").unwrap();
    assert_eq!(doc.source(), source.replace("item \"d\"", "item \"D\""));
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
}

#[test]
fn root_metadata_and_slide_constraints_are_schema_errors() {
    for source in [
        "presentation;",
        "presentation { slide; }",
        "presentation 1 { metadata { title \"x\"; }; }",
        "presentation extra=1 { metadata { title \"x\"; }; }",
        "presentation { metadata; }",
        "presentation { metadata { title \"x\"; }; metadata { title \"y\"; }; }",
        "presentation { slide; metadata { title \"x\"; }; }",
        "presentation { metadata 1 { title \"x\"; }; }",
        "presentation { metadata attr=1 { title \"x\"; }; }",
        "presentation { metadata { title \"x\"; text \"y\"; }; }",
        "presentation { metadata { title; }; }",
        "presentation { metadata { title \"\"; }; }",
        "presentation { metadata { title 1; }; }",
        "presentation { metadata { title \"x\" \"y\"; }; }",
        "presentation { metadata { title \"x\" extra=1; }; }",
        "presentation { metadata { title \"x\" { child; }; }; }",
        "presentation { metadata { title \"x\"; }; slide 1; }",
        "presentation { metadata { title \"x\"; }; slide id=1; }",
        "presentation { metadata { title \"x\"; }; slide extra=1; }",
        "presentation { metadata { title \"x\"; }; slide id=\"a\" id=\"b\"; }",
        "presentation { metadata { title \"x\"; }; slide id=\"a\"; slide id=\"a\"; }",
        "presentation { metadata { title \"x\"; }; unknown; }",
        "presentation { metadata { title \"x\"; }; }; presentation;",
    ] {
        let doc = PresentationDocument::from_source(source).unwrap();
        assert!(!doc.can_edit(), "{source}");
        assert!(!doc.diagnostics().is_empty());
        assert!(
            doc.diagnostics()
                .iter()
                .all(|d| d.kind == psycho::DiagnosticKind::Schema),
            "{source}: {:?}",
            doc.diagnostics()
        );
    }
}

#[test]
fn root_to_column_moves_work_at_every_valid_insertion_position() {
    let source = "presentation { metadata { title \"test\"; }; slide { text \"a\"; text \"b\"; text \"c\"; columns { column width=50 { text \"left-1\"; text \"left-2\"; }; column width=50 { text \"right-1\"; text \"right-2\"; }; }; }; }";
    for from in 0..3 {
        for column in 0..2 {
            for position in 0..=2 {
                let mut doc = PresentationDocument::from_source(source).unwrap();
                let mut expected = doc.model().unwrap().clone();
                let moving = expected.slides[0].elements.remove(from);
                let Element::Columns { left, right, .. } = &mut expected.slides[0].elements[2]
                else {
                    panic!()
                };
                if column == 0 { left } else { right }.insert(position, moving);
                doc.move_element_to_column_at(0, from, 3, column, position)
                    .unwrap();
                assert_eq!(doc.model().unwrap(), &expected);
                doc.move_column_element_to_slide_at(0, 2, column, position, from)
                    .unwrap();
                assert_eq!(
                    doc.model().unwrap(),
                    PresentationDocument::from_source(source)
                        .unwrap()
                        .model()
                        .unwrap()
                );
                doc.undo();
                doc.undo();
                assert_eq!(doc.source(), source);
            }
        }
    }
}

#[test]
fn column_moves_change_only_the_requested_element_position() {
    let source = r#"presentation {
    metadata { title "unchanged" }
    slide id="first" {
        heading "untouched"
        columns {
            column width=40 { text "a"; code "b" language="rust"; }
            column width=60 { image "c.png" { caption "caption"; }; }
        }
        columns {
            column width=30 { bullets { item "d"; item "e"; }; }
            column width=70 { text "f"; }
        }
    }
    slide id="second" { text "other slide"; }
}"#;
    for operation in 0..3 {
        let mut document = PresentationDocument::from_source(source).unwrap();
        let mut expected = document.model().unwrap().clone();
        let Element::Columns { left, right, .. } = &mut expected.slides[0].elements[1] else {
            panic!()
        };
        let moving = left.remove(0);
        match operation {
            0 => {
                left.insert(1, moving);
                document.move_column_element(0, 1, 0, 0, 1).unwrap();
            }
            1 => {
                right.insert(0, moving);
                document
                    .move_column_element_to_column_at(0, 1, 0, 0, 1, 0)
                    .unwrap();
            }
            _ => {
                let Element::Columns { right, .. } = &mut expected.slides[0].elements[2] else {
                    panic!()
                };
                right.insert(0, moving);
                document
                    .move_column_element_between_columns_at(0, 1, 0, 0, 2, 1, 0)
                    .unwrap();
            }
        }
        assert_eq!(document.model().unwrap(), &expected);
        assert!(document.undo());
        assert_eq!(document.source(), source);
        assert!(document.redo());
        assert_eq!(document.model().unwrap(), &expected);
    }
}

#[test]
fn loaded_images_composite_transparency_on_white_and_apply_exif_orientation() {
    let dir = tempfile::tempdir().unwrap();
    let source = "presentation { metadata { title \"images\"; }; slide { image \"asset.bin\"; }; }";
    let doc = PresentationDocument::from_source_with_asset_base(source, dir.path()).unwrap();
    for format in [image::ImageFormat::Png, image::ImageFormat::WebP] {
        let pixels = image::RgbaImage::from_fn(3, 1, |x, _| match x {
            0 => image::Rgba([10, 20, 30, 0]),
            1 => image::Rgba([10, 20, 30, 128]),
            _ => image::Rgba([10, 20, 30, 255]),
        });
        pixels
            .save_with_format(dir.path().join("asset.bin"), format)
            .unwrap();
        let assets = doc.load_assets();
        assert!(assets.diagnostics.is_empty(), "{format:?}");
        let loaded = &assets.images["asset.bin"];
        assert_eq!(loaded.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(loaded.get_pixel(1, 0).0, [132, 137, 142, 255]);
        assert_eq!(loaded.get_pixel(2, 0).0, [10, 20, 30, 255]);
    }
    fs::write(
        dir.path().join("asset.bin"),
        include_bytes!("fixtures/orientation-6.jpg"),
    )
    .unwrap();
    let assets = doc.load_assets();
    assert!(assets.diagnostics.is_empty());
    assert_eq!(assets.images["asset.bin"].dimensions(), (40, 80));
}

#[test]
fn asset_snapshots_share_failures_and_preserve_edits_and_history_during_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let source = "presentation { metadata { title \"images\"; }; slide { image \"asset.bin\" { caption \"keep\"; }; image \"asset.bin\"; }; }";
    let mut doc = PresentationDocument::from_source_with_asset_base(source, dir.path()).unwrap();
    image::RgbImage::from_pixel(2, 1, image::Rgb([1, 2, 3]))
        .save_with_format(dir.path().join("asset.bin"), image::ImageFormat::Png)
        .unwrap();
    doc.set_element_field(0, 0, ElementField::Caption, "draft")
        .unwrap();
    let edited = doc.source().to_owned();
    let loaded = doc.load_assets();
    assert_eq!(loaded.images.len(), 1);
    assert!(loaded.diagnostics.is_empty());
    fs::remove_file(dir.path().join("asset.bin")).unwrap();
    assert_eq!(loaded.images["asset.bin"].get_pixel(0, 0).0, [1, 2, 3, 255]);
    let missing = doc.load_assets();
    assert!(missing.images.is_empty());
    assert_eq!(missing.diagnostics.len(), 2);
    assert!(
        missing
            .diagnostics
            .iter()
            .all(|d| d.problem == psycho::AssetProblem::Missing)
    );
    fs::write(
        dir.path().join("asset.bin"),
        include_bytes!("fixtures/animated.png"),
    )
    .unwrap();
    let animated = doc.load_assets();
    assert!(animated.images.is_empty());
    assert!(
        animated
            .diagnostics
            .iter()
            .all(|d| d.problem == psycho::AssetProblem::AnimatedImage)
    );
    image::RgbImage::from_pixel(1, 2, image::Rgb([4, 5, 6]))
        .save_with_format(dir.path().join("asset.bin"), image::ImageFormat::WebP)
        .unwrap();
    let recovered = doc.load_assets();
    assert!(recovered.diagnostics.is_empty());
    assert_eq!(recovered.images["asset.bin"].dimensions(), (1, 2));
    assert_eq!(doc.source(), edited);
    assert!(doc.can_undo());
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
    assert!(doc.redo());
    assert_eq!(doc.source(), edited);
}

#[test]
fn image_path_history_loads_the_current_asset_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let source = "presentation { metadata { title \"images\"; }; slide { image \"a.bin\"; }; }";
    let mut doc = PresentationDocument::from_source_with_asset_base(source, dir.path()).unwrap();
    for (path, color) in [("a.bin", [1, 2, 3]), ("b.bin", [4, 5, 6])] {
        image::RgbImage::from_pixel(1, 1, image::Rgb(color))
            .save_with_format(dir.path().join(path), image::ImageFormat::Png)
            .unwrap();
    }
    doc.set_element_field(0, 0, ElementField::ImagePath, "b.bin")
        .unwrap();
    assert_eq!(
        doc.load_assets().images["b.bin"].get_pixel(0, 0).0,
        [4, 5, 6, 255]
    );
    image::RgbImage::from_pixel(1, 1, image::Rgb([7, 8, 9]))
        .save_with_format(dir.path().join("a.bin"), image::ImageFormat::Png)
        .unwrap();
    assert!(doc.undo());
    assert_eq!(
        doc.load_assets().images["a.bin"].get_pixel(0, 0).0,
        [7, 8, 9, 255]
    );
    fs::remove_file(dir.path().join("b.bin")).unwrap();
    assert!(doc.redo());
    let assets = doc.load_assets();
    assert!(assets.images.is_empty());
    assert_eq!(assets.diagnostics[0].path, "b.bin");
    assert_eq!(assets.diagnostics[0].problem, psycho::AssetProblem::Missing);
}
