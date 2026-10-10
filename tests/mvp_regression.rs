use psycho::{DiagnosticKind, ElementField, ElementKind, PresentationDocument};
use std::fs;

#[test]
fn prototype_01_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "﻿presentation {\r\n\tmetadata { title \"例\" }\r\n\tslide { text #\"日本\"# } \r\n\tslide { image \"assets/a.png\" }\r\n}";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.save().unwrap();
    let expected = PresentationDocument::from_source("﻿presentation {\r\n\tmetadata { title \"例\" }\r\n\tslide { text #\"日本\"# } \r\n\tslide { image \"assets/a.png\" }\r\n}").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_01_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "﻿presentation {\r\n\tmetadata { title \"例\" }\r\n\tslide { text #\"日本\"# } \r\n\tslide { image \"assets/a.png\" }\r\n}";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.set_element_text(0, 0, "日本語の新しい本文\n次の行")
        .unwrap();
    let expected = PresentationDocument::from_source("﻿presentation {\r\n\tmetadata { title \"例\" }\r\n\tslide { text \"日本語の新しい本文\\n次の行\" } \r\n\tslide { image \"assets/a.png\" }\r\n}").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_01_03() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "﻿presentation {\r\n\tmetadata { title \"例\" }\r\n\tslide { text \"日本語の新しい本文\\n次の行\" } \r\n\tslide { image \"assets/a.png\" }\r\n}";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.set_element_field(1, 0, ElementField::ImagePath, "assets/新しい画像.png")
        .unwrap();
    let expected = PresentationDocument::from_source("﻿presentation {\r\n\tmetadata { title \"例\" }\r\n\tslide { text \"日本語の新しい本文\\n次の行\" } \r\n\tslide { image \"assets/新しい画像.png\" }\r\n}").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_02_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"コメント\" }\n    slide {\n        // 独立コメント: この場所に残す\n        image \"a.png\" {\n            // 内部コメント: 画像に伴う\n            caption \"図\"\n        }; // 末尾コメント: 画像に伴う\n        text \"残す\"; text \"同じ行\"; // 二つ目に伴う\n        columns {\n            column width=50 {\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 3, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"コメント\" }\n    slide {\n        // 独立コメント: この場所に残す\n        text \"残す\"; text \"同じ行\"; // 二つ目に伴う\n        columns {\n            column width=50 {\n                image \"a.png\" {\n                    // 内部コメント: 画像に伴う\n                    caption \"図\"\n                }; // 末尾コメント: 画像に伴う\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_02_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"コメント\" }\n    slide {\n        // 独立コメント: この場所に残す\n        text \"残す\"; text \"同じ行\"; // 二つ目に伴う\n        columns {\n            column width=50 {\n                image \"a.png\" {\n                    // 内部コメント: 画像に伴う\n                    caption \"図\"\n                }; // 末尾コメント: 画像に伴う\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.remove_column_element(0, 2, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"コメント\" }\n    slide {\n        // 独立コメント: この場所に残す\n        text \"残す\"; text \"同じ行\"; // 二つ目に伴う\n        columns {\n            column width=50 {\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_02_03() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"コメント\" }\n    slide {\n        // 独立コメント: この場所に残す\n        text \"残す\"; text \"同じ行\"; // 二つ目に伴う\n        columns {\n            column width=50 {\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 1, 2, 1, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"コメント\" }\n    slide {\n        // 独立コメント: この場所に残す\n        text \"残す\";         columns {\n            column width=50 {\n            }\n            column width=50 {\n                text \"同じ行\"; // 二つ目に伴う\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_03_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        code language=\"rust\" \"引用: \\\"x\\\" と改行\\n日本語\"\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" \"引用: \\\"x\\\" と改行\\n日本語\"\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_03_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" \"引用: \\\"x\\\" と改行\\n日本語\"\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_column_element_to_slide_at(0, 0, 0, 0, 1).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n        code language=\"rust\" \"引用: \\\"x\\\" と改行\\n日本語\"\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_04_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        code language=\"rust\" #\"C:\\path\\file \"引用\"\"#\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" #\"C:\\path\\file \"引用\"\"#\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_04_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" #\"C:\\path\\file \"引用\"\"#\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_column_element_to_slide_at(0, 0, 0, 0, 1).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n        code language=\"rust\" #\"C:\\path\\file \"引用\"\"#\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_05_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        code language=\"rust\" \"\"\"\n            fn main() {\n                println!(\"日本\");\n            }\n\n            \"\"\"\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" \"\"\"\n            fn main() {\n                println!(\"日本\");\n            }\n\n            \"\"\"\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_05_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" \"\"\"\n                    fn main() {\n                        println!(\"日本\");\n                    }\n\n                    \"\"\"\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_column_element_to_slide_at(0, 0, 0, 0, 1).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n        code language=\"rust\" \"\"\"\n                    fn main() {\n                        println!(\"日本\");\n                    }\n\n                    \"\"\"\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_06_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        code language=\"rust\" #\"\"\"\n            \\n はそのまま\n                二行目\n            \"\"\"#\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" #\"\"\"\n            \\n はそのまま\n                二行目\n            \"\"\"#\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_06_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n                code language=\"rust\" #\"\"\"\n                    \\n はそのまま\n                        二行目\n                    \"\"\"#\n            }\n            column width=40 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_column_element_to_slide_at(0, 0, 0, 0, 1).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"移動\" }\n    slide {\n        columns {\n            column width=60 {\n            }\n            column width=40 {\n            }\n        }\n        code language=\"rust\" #\"\"\"\n                    \\n はそのまま\n                        二行目\n                    \"\"\"#\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_07_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\r\n\tmetadata { title \"追加\" }\r\n\tslide {\r\n\t\ttext \"既存\"\r\n\t}\r\n\tslide {}\r\n}\r\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    let index = doc.add_element(0, ElementKind::Text).unwrap();
    doc.set_element_text(0, index, "追加").unwrap();
    let expected = PresentationDocument::from_source("presentation {\r\n\tmetadata { title \"追加\" }\r\n\tslide {\r\n\t\ttext \"既存\"\r\n\t\ttext \"追加\"\r\n\t}\r\n\tslide {}\r\n}\r\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_07_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\r\n\tmetadata { title \"追加\" }\r\n\tslide {\r\n\t\ttext \"既存\"\r\n\t\ttext \"追加\"\r\n\t}\r\n\tslide {}\r\n}\r\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    let index = doc.add_element(1, ElementKind::Heading).unwrap();
    doc.set_element_text(1, index, "空の親へ").unwrap();
    let expected = PresentationDocument::from_source("presentation {\r\n\tmetadata { title \"追加\" }\r\n\tslide {\r\n\t\ttext \"既存\"\r\n\t\ttext \"追加\"\r\n\t}\r\n\tslide {\r\n\t\theading \"空の親へ\"\r\n\t}\r\n}\r\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_07_03() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\r\n\tmetadata { title \"追加\" }\r\n\tslide {\r\n\t\ttext \"既存\"\r\n\t\ttext \"追加\"\r\n\t}\r\n\tslide {\r\n\t\theading \"空の親へ\"\r\n\t}\r\n}\r\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    let index = doc.add_element(0, ElementKind::Text).unwrap();
    doc.set_element_text(0, index, "先頭").unwrap();
    doc.move_element(0, index, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\r\n\tmetadata { title \"追加\" }\r\n\tslide {\r\n\t\ttext \"先頭\"\r\n\t\ttext \"既存\"\r\n\t\ttext \"追加\"\r\n\t}\r\n\tslide {\r\n\t\theading \"空の親へ\"\r\n\t}\r\n}\r\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_08_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ \"rust\" /-language=\"ignored\" \"日本\" }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.set_element_field(0, 0, ElementField::CodeLanguage, "javascript")
        .unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ \"javascript\" /-language=\"ignored\" \"日本\" }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_08_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ javascript /-language=\"ignored\" \"日本\" }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.set_element_text(0, 0, "日本語を長くする\n二行目")
        .unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ javascript /-language=\"ignored\" \"日本語を長くする\\n二行目\" }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_08_03() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ javascript /-language=\"ignored\" \"日本語を長くする\\n二行目\" }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    let index = doc.add_element(0, ElementKind::Text).unwrap();
    doc.set_element_text(0, index, "追加").unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ javascript /-language=\"ignored\" \"日本語を長くする\\n二行目\" \n        text \"追加\"\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_08_04() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ javascript /-language=\"ignored\" \"日本語を長くする\\n二行目\" language=\"python\" \n        text \"追加\"\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.model().is_none());
    assert!(!doc.can_edit());
    assert!(doc
        .diagnostics()
        .iter()
        .any(|d| d.kind == DiagnosticKind::Schema && d.source_line.contains("language")));
    assert!(doc.save().is_err());
    assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
}

#[test]
fn prototype_09_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"不均一なインデント\" }\n    slide {\n        code \"\"\"\n        significant spaces\nzero indent\n\"\"\"\n        columns {\n            column width=50 {\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"不均一なインデント\" }\n    slide {\n        columns {\n            column width=50 {\n                code \"\"\"\n        significant spaces\nzero indent\n\"\"\"\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_09_03() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\n    metadata { title \"不均一なインデント\" }\n    slide {\n        columns {\n            column width=50 {\n                code \"\"\"\n        significant spaces\nzero indent\n\"\"\"\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_column_element_to_slide_at(0, 0, 0, 0, 1).unwrap();
    let expected = PresentationDocument::from_source("presentation {\n    metadata { title \"不均一なインデント\" }\n    slide {\n        columns {\n            column width=50 {\n            }\n            column width=50 {\n            }\n        }\n        code \"\"\"\n        significant spaces\nzero indent\n\"\"\"\n    }\n}\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_10_01() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\r\n\tmetadata { title \"字句の区別\" }\r\n\tslide {\r\n\t\tcode /* 外側 \" /* 内側 */ */ /-#\"\"\"\r\n\t\t無効化した文字列\r\n\"\"\"# #\"\"\"\r\n\t\t実際の文字列\r\n\"\"\"#; // 伴うコメント\r\n\t\tcolumns {\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t}\r\n\t}\r\n}\r\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let expected = PresentationDocument::from_source("presentation {\r\n\tmetadata { title \"字句の区別\" }\r\n\tslide {\r\n\t\tcolumns {\r\n\t\t\tcolumn width=50 {\r\n\t\t\t\tcode /* 外側 \" /* 内側 */ */ /-#\"\"\"\r\n\t\t無効化した文字列\r\n\"\"\"# #\"\"\"\r\n\t\t実際の文字列\r\n\"\"\"#; // 伴うコメント\r\n\t\t\t}\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t}\r\n\t}\r\n}\r\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn prototype_10_02() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let source = "presentation {\r\n\tmetadata { title \"字句の区別\" }\r\n\tslide {\r\n\t\tcolumns {\r\n\t\t\tcolumn width=50 {\r\n\t\t\t\tcode /* 外側 \" /* 内側 */ */ /-#\"\"\"\r\n\t\t無効化した文字列\r\n\"\"\"# #\"\"\"\r\n\t\t実際の文字列\r\n\"\"\"#; // 伴うコメント\r\n\t\t\t}\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t}\r\n\t}\r\n}\r\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_column_element_to_slide_at(0, 0, 0, 0, 1).unwrap();
    let expected = PresentationDocument::from_source("presentation {\r\n\tmetadata { title \"字句の区別\" }\r\n\tslide {\r\n\t\tcolumns {\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t}\r\n\t\tcode /* 外側 \" /* 内側 */ */ /-#\"\"\"\r\n\t\t無効化した文字列\r\n\"\"\"# #\"\"\"\r\n\t\t実際の文字列\r\n\"\"\"#; // 伴うコメント\r\n\t}\r\n}\r\n").unwrap();
    assert_eq!(doc.model(), expected.model());
    assert_eq!(doc.source(), expected.source());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    let reopened = PresentationDocument::open(&path).unwrap();
    assert_eq!(reopened.model(), expected.model());
    assert_eq!(reopened.source(), expected.source());
    while doc.undo() {}
    assert_eq!(doc.source(), source);
    assert_eq!(fs::read(&path).unwrap(), expected.source().as_bytes());
    while doc.redo() {}
    assert_eq!(doc.source(), expected.source());
}

#[test]
fn schema_node_contracts_cover_unchecked_headers_children_and_empty_values() {
    let invalid = [
        "heading 1",
        "heading ",
        "heading \"a\" \"b\"",
        "heading \"a\" { text \"b\"; }",
        "text",
        "text \"a\" attr=1",
        "bullets attr=1",
        "bullets { item; }",
        "bullets { item \"a\" \"b\"; }",
        "bullets { item \"a\" attr=1; }",
        "bullets { item \"a\" { text \"b\"; }; }",
        "code",
        "code 1",
        "code \"a\" \"b\"",
        "code \"a\" attr=1",
        "code \"a\" { text \"b\"; }",
        "image",
        "image \"a.png\" \"b.png\"",
        "image \"a.png\" attr=1",
        "image \"a.png\" { caption; }",
        "image \"a.png\" { caption \"a\" \"b\"; }",
        "image \"a.png\" { caption \"a\" attr=1; }",
        "image \"a.png\" { caption \"a\" { text \"b\"; }; }",
        "columns 1 { column width=50; column width=50; }",
        "columns attr=1 { column width=50; column width=50; }",
        "columns { text \"a\"; column width=50; }",
        "columns { column 1 width=50; column width=50; }",
        "columns { column width=50 attr=1; column width=50; }",
        "columns { column width=\"50\"; column width=50; }",
        "columns { column width=-1; column width=101; }",
        "columns { column width=50 { item \"a\"; }; column width=50; }",
    ];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema.kdl");
    for element in invalid {
        let source =
            format!("presentation {{ metadata {{ title \"schema\"; }}; slide {{ {element}; }}; }}");
        fs::write(&path, &source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(doc.model().is_none(), "{element}");
        assert!(!doc.can_edit(), "{element}");
        assert!(!doc.diagnostics().is_empty(), "{element}");
        assert!(
            doc.diagnostics()
                .iter()
                .all(|d| d.kind == DiagnosticKind::Schema),
            "{element}"
        );
        assert!(doc.save().is_err(), "{element}");
        assert_eq!(fs::read(&path).unwrap(), source.as_bytes(), "{element}");
    }
    for source in [
        "",
        "presentation { metadata { title \"a\"; title \"b\"; }; }",
        "unknown;",
    ] {
        fs::write(&path, source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(doc.model().is_none(), "{source}");
        assert!(doc
            .diagnostics()
            .iter()
            .all(|d| d.kind == DiagnosticKind::Schema));
        assert!(doc.save().is_err());
        assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
    }
    for source in [
        "presentation { metadata { title \"valid\"; }; }",
        "presentation { metadata { title \"valid\"; }; slide id=\"\" { heading \"\"; text \"\"; bullets {}; bullets { item \"\"; }; code \"\" language=\"\"; image \"../asset.png\" { caption \"\"; }; columns { column width=1 {}; column width=99 {}; }; }; }",
    ] {
        fs::write(&path, source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(doc.can_edit(), "{:?}", doc.diagnostics());
        doc.save().unwrap();
        assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
    }
}

#[test]
fn deleting_images_preserves_shared_asset_bytes_and_exact_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("presentation.kdl");
    let asset = dir.path().join("shared.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([12, 34, 56, 255]))
        .save(&asset)
        .unwrap();
    let asset_bytes = fs::read(&asset).unwrap();
    let source = "presentation { metadata { title \"images\"; }; slide { image \"shared.png\" { caption \"root\"; }; columns { column width=50 { image \"shared.png\" { caption \"nested\"; }; }; column width=50; }; }; }\r\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    doc.remove_element(0, 0).unwrap();
    doc.remove_column_element(0, 0, 0, 0).unwrap();
    let deleted = "presentation { metadata { title \"images\"; }; slide {  columns { column width=50 {  }; column width=50; }; }; }\r\n";
    assert_eq!(doc.source(), deleted);
    assert!(doc.model().unwrap().slides[0].image_paths().is_empty());
    doc.save().unwrap();
    assert_eq!(fs::read(&asset).unwrap(), asset_bytes);
    assert_eq!(fs::read(&path).unwrap(), deleted.as_bytes());
    assert_eq!(PresentationDocument::open(&path).unwrap().source(), deleted);
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(doc.source(), source);
    assert_eq!(doc.load_assets().images["shared.png"].dimensions(), (2, 1));
    assert_eq!(fs::read(&path).unwrap(), deleted.as_bytes());
    assert!(doc.redo());
    assert!(doc.redo());
    assert_eq!(doc.source(), deleted);
    assert_eq!(fs::read(&asset).unwrap(), asset_bytes);
}
