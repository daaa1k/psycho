use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::Cursor;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticKind {
    Syntax,
    Schema,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetProblem {
    Missing,
    PermissionDenied,
    UnsupportedFormat,
    AnimatedImage,
    Corrupt,
}

impl fmt::Display for AssetProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Missing => "画像ファイルがありません",
            Self::PermissionDenied => "画像を読み取る権限がありません",
            Self::UnsupportedFormat => "PNG・JPEG・WebP 以外の画像形式です",
            Self::AnimatedImage => "アニメーション画像には対応していません",
            Self::Corrupt => "画像をデコードできません",
        };
        f.write_str(label)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetDiagnostic {
    pub path: String,
    pub problem: AssetProblem,
    pub slide_index: usize,
    pub slide_id: Option<String>,
    pub element_index: usize,
    pub column_index: Option<usize>,
    pub nested_element_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub file_name: Option<String>,
    pub line: usize,
    pub column: usize,
    pub source_line: String,
    pub slide_index: Option<usize>,
    pub slide_id: Option<String>,
    pub element_index: Option<usize>,
    pub column_index: Option<usize>,
    pub nested_element_index: Option<usize>,
    pub byte_range: std::ops::Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationModel {
    pub title: String,
    pub slides: Vec<Slide>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Slide {
    pub id: Option<String>,
    pub elements: Vec<Element>,
}

impl Slide {
    pub fn image_paths(&self) -> Vec<&str> {
        let mut paths = Vec::new();
        for element in &self.elements {
            collect_image_paths(element, &mut paths);
        }
        paths
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Element {
    Heading(String),
    Text(String),
    Bullets(Vec<String>),
    Code {
        text: String,
        language: Option<String>,
    },
    Image {
        path: String,
        caption: Option<String>,
    },
    Columns {
        left_width: u8,
        right_width: u8,
        left: Vec<Element>,
        right: Vec<Element>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementKind {
    Heading,
    Text,
    Bullets,
    Code,
    Image,
    Columns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementField {
    Text,
    ImagePath,
    CodeLanguage,
    Bullet(usize),
    Caption,
}

#[derive(Debug)]
pub enum DocumentError {
    Io(std::io::Error),
    NoPath,
    InvalidDocument,
    ExternalChange,
    DestinationExists,
    UnsafeValue(String),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::NoPath => write!(f, "document has no file path"),
            Self::InvalidDocument => write!(f, "document has syntax or schema errors"),
            Self::ExternalChange => write!(f, "external change detected; save was stopped"),
            Self::DestinationExists => {
                write!(f, "destination already exists; overwrite was stopped")
            }
            Self::UnsafeValue(message) => write!(f, "value was rejected: {message}"),
        }
    }
}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DocumentError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// A source-preserving editor model. Public operations are the shared seam for
/// the GUI and persistence tests; parsed KDL values are never serialized as a
/// whole document when a user edits one field.
#[derive(Clone)]
pub struct PresentationDocument {
    source: String,
    saved_source: String,
    path: Option<PathBuf>,
    asset_base: Option<PathBuf>,
    model: Option<PresentationModel>,
    diagnostics: Vec<Diagnostic>,
    undo: Vec<String>,
    redo: Vec<String>,
    undo_labels: Vec<String>,
    redo_labels: Vec<String>,
}

impl PresentationDocument {
    pub fn from_source(source: &str) -> Result<Self, DocumentError> {
        Ok(Self::from_source_at(
            source.to_owned(),
            None,
            source.to_owned(),
            None,
        ))
    }

    pub fn from_source_with_asset_base(
        source: &str,
        asset_base: impl AsRef<Path>,
    ) -> Result<Self, DocumentError> {
        Ok(Self::from_source_at(
            source.to_owned(),
            None,
            source.to_owned(),
            Some(asset_base.as_ref().to_path_buf()),
        ))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path = path.as_ref().to_path_buf();
        let mut bytes = Vec::new();
        fs::File::open(&path)?.read_to_end(&mut bytes)?;
        let source = String::from_utf8(bytes).map_err(|error| {
            DocumentError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        })?;
        let asset_base = path.parent().map(Path::to_path_buf);
        Ok(Self::from_source_at(
            source.clone(),
            Some(path),
            source,
            asset_base,
        ))
    }

    fn from_source_at(
        source: String,
        path: Option<PathBuf>,
        saved_source: String,
        asset_base: Option<PathBuf>,
    ) -> Self {
        let (model, diagnostics) = parse_and_validate(&source, path.as_deref());
        Self {
            source,
            saved_source,
            path,
            asset_base,
            model,
            diagnostics,
            undo: Vec::new(),
            redo: Vec::new(),
            undo_labels: Vec::new(),
            redo_labels: Vec::new(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn asset_base(&self) -> &Path {
        self.asset_base
            .as_deref()
            .or_else(|| self.path.as_deref().and_then(Path::parent))
            .unwrap_or(Path::new("."))
    }

    pub fn model(&self) -> Option<&PresentationModel> {
        self.model.as_ref()
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn can_edit(&self) -> bool {
        self.model.is_some() && self.diagnostics.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        self.source != self.saved_source
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_description(&self) -> Option<&str> {
        self.undo_labels.last().map(String::as_str)
    }

    pub fn redo_description(&self) -> Option<&str> {
        self.redo_labels.last().map(String::as_str)
    }

    pub fn inspect_assets(&self) -> Vec<AssetDiagnostic> {
        let Some(model) = self.model.as_ref() else {
            return Vec::new();
        };
        let base = self.asset_base();
        let mut diagnostics = Vec::new();
        for (slide_index, slide) in model.slides.iter().enumerate() {
            for (element_index, element) in slide.elements.iter().enumerate() {
                match element {
                    Element::Image { path, .. } => push_asset_diagnostic(
                        &mut diagnostics,
                        &base,
                        path,
                        slide,
                        slide_index,
                        element_index,
                        None,
                        None,
                    ),
                    Element::Columns { left, right, .. } => {
                        for (column_index, elements) in [(0, left), (1, right)] {
                            for (nested_element_index, element) in elements.iter().enumerate() {
                                if let Element::Image { path, .. } = element {
                                    push_asset_diagnostic(
                                        &mut diagnostics,
                                        &base,
                                        path,
                                        slide,
                                        slide_index,
                                        element_index,
                                        Some(column_index),
                                        Some(nested_element_index),
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        diagnostics
    }

    pub fn set_title(&mut self, title: &str) -> Result<(), DocumentError> {
        if title.is_empty() {
            return Err(DocumentError::UnsafeValue("title must not be empty".into()));
        }
        let parsed = self.editable_document()?;
        let root = parsed
            .nodes()
            .first()
            .ok_or(DocumentError::InvalidDocument)?;
        let metadata = child_named(root, "metadata", 0).ok_or(DocumentError::InvalidDocument)?;
        let title_node = child_named(metadata, "title", 0).ok_or(DocumentError::InvalidDocument)?;
        let entry = title_node
            .entries()
            .first()
            .ok_or(DocumentError::InvalidDocument)?;
        self.replace_entry_string(entry, title)
    }

    pub fn set_element_text(
        &mut self,
        slide_index: usize,
        element_index: usize,
        text: &str,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let root = parsed
            .nodes()
            .first()
            .ok_or(DocumentError::InvalidDocument)?;
        let slide =
            child_named(root, "slide", slide_index).ok_or(DocumentError::InvalidDocument)?;
        let elements = slide.children().ok_or(DocumentError::InvalidDocument)?;
        let node = elements
            .nodes()
            .get(element_index)
            .ok_or(DocumentError::InvalidDocument)?;
        let field = match node.name().value() {
            "image" => ElementField::ImagePath,
            _ => ElementField::Text,
        };
        self.set_element_field(slide_index, element_index, field, text)
    }

    pub fn set_element_field(
        &mut self,
        slide_index: usize,
        element_index: usize,
        field: ElementField,
        value: &str,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let slide = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let node = slide
            .children()
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        let entry = match field {
            ElementField::Text if matches!(node.name().value(), "heading" | "text" | "code") => {
                node.entries().iter().find(|entry| entry.name().is_none())
            }
            ElementField::ImagePath if node.name().value() == "image" => {
                node.entries().iter().find(|entry| entry.name().is_none())
            }
            ElementField::CodeLanguage if node.name().value() == "code" => node
                .entries()
                .iter()
                .find(|entry| entry.name().is_some_and(|name| name.value() == "language")),
            ElementField::Bullet(index) if node.name().value() == "bullets" => node
                .children()
                .and_then(|children| children.nodes().get(index))
                .and_then(|item| item.entries().first()),
            ElementField::Caption if node.name().value() == "image" => node
                .children()
                .and_then(|children| {
                    children
                        .nodes()
                        .iter()
                        .find(|child| child.name().value() == "caption")
                })
                .and_then(|caption| caption.entries().first()),
            _ => None,
        };
        if let Some(entry) = entry {
            return self.replace_entry_string(entry, value);
        }
        if field == ElementField::CodeLanguage && node.name().value() == "code" {
            return self.insert_code_language(node, value);
        }
        if field == ElementField::Caption && node.name().value() == "image" {
            let caption = format!("caption {}", encode_kdl_string(value));
            return self.append_child(node, &caption);
        }
        Err(DocumentError::UnsafeValue(
            "selected element has no matching text field".into(),
        ))
    }

    pub fn set_bullets_text(
        &mut self,
        slide_index: usize,
        element_index: usize,
        text: &str,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let slide = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let node = slide
            .children()
            .and_then(|children| children.nodes().get(element_index))
            .filter(|node| node.name().value() == "bullets")
            .ok_or_else(|| DocumentError::UnsafeValue("selected element is not bullets".into()))?;
        let requested = text.split('\n').collect::<Vec<_>>();
        let existing = node
            .children()
            .map(|children| children.nodes())
            .unwrap_or(&[]);
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        for (item, value) in existing.iter().zip(requested.iter()) {
            let entry = item
                .entries()
                .first()
                .ok_or(DocumentError::InvalidDocument)?;
            let (start, end) = entry_string_range(&self.source, entry)?;
            edits.push((start, end, encode_kdl_string(value)));
        }
        for item in existing.iter().skip(requested.len()) {
            edits.push((
                node_source_start(&self.source, item),
                item.span().offset() + item.span().len(),
                String::new(),
            ));
        }
        if requested.len() > existing.len() {
            let missing = requested
                .iter()
                .skip(existing.len())
                .map(|value| format!("item {}", encode_kdl_string(value)))
                .collect::<Vec<_>>();
            let insertion = child_insertion(&self.source, node, &missing.join("\n"))?;
            edits.push(insertion);
        }
        edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
        let mut candidate = self.source.clone();
        for (start, end, replacement) in edits {
            if end > candidate.len()
                || start > end
                || !candidate.is_char_boundary(start)
                || !candidate.is_char_boundary(end)
            {
                return Err(DocumentError::UnsafeValue(
                    "invalid source range while editing bullets".into(),
                ));
            }
            candidate.replace_range(start..end, &replacement);
        }
        self.commit_candidate(candidate)
    }

    pub fn add_bullet(
        &mut self,
        slide_index: usize,
        element_index: usize,
        item_index: usize,
        value: &str,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let bullets = slide_node(&parsed, slide_index)
            .and_then(|slide| slide.children())
            .and_then(|children| children.nodes().get(element_index))
            .filter(|node| node.name().value() == "bullets")
            .ok_or_else(|| DocumentError::UnsafeValue("selected element is not bullets".into()))?;
        let items = bullets
            .children()
            .map(|children| children.nodes())
            .unwrap_or(&[]);
        if item_index > items.len() {
            return Err(DocumentError::UnsafeValue(
                "bullet position is outside the list".into(),
            ));
        }
        let addition = format!("item {}", encode_kdl_string(value));
        let mut candidate = self.source.clone();
        if item_index == items.len() {
            let (start, end, replacement) = child_insertion(&self.source, bullets, &addition)?;
            candidate.replace_range(start..end, &replacement);
        } else {
            let item = &items[item_index];
            let (start, _) = node_line_range(&self.source, item)?;
            let indent = line_indent(&self.source, item.name().span().offset());
            let line_ending = line_ending_near(&self.source, start);
            candidate.insert_str(start, &format!("{indent}{addition}{line_ending}"));
        }
        self.commit_candidate(candidate)
    }

    pub fn remove_bullet(
        &mut self,
        slide_index: usize,
        element_index: usize,
        item_index: usize,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let item = slide_node(&parsed, slide_index)
            .and_then(|slide| slide.children())
            .and_then(|children| children.nodes().get(element_index))
            .filter(|node| node.name().value() == "bullets")
            .and_then(|node| node.children())
            .and_then(|children| children.nodes().get(item_index))
            .ok_or_else(|| {
                DocumentError::UnsafeValue("bullet position is outside the list".into())
            })?;
        self.remove_node(item)
    }

    pub fn move_bullet(
        &mut self,
        slide_index: usize,
        element_index: usize,
        from: usize,
        to: usize,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let bullets = slide_node(&parsed, slide_index)
            .and_then(|slide| slide.children())
            .and_then(|children| children.nodes().get(element_index))
            .filter(|node| node.name().value() == "bullets")
            .ok_or_else(|| DocumentError::UnsafeValue("selected element is not bullets".into()))?;
        let items = bullets
            .children()
            .map(|children| children.nodes())
            .unwrap_or(&[]);
        if from >= items.len() || to >= items.len() {
            return Err(DocumentError::UnsafeValue(
                "bullet position is outside the list".into(),
            ));
        }
        if from == to {
            return Ok(());
        }
        let (start, end) = node_line_range(&self.source, &items[from])?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");

        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let parent = slide_node(&reparsed, slide_index)
            .and_then(|slide| slide.children())
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        insert_moved_source_at(&mut candidate, parent, to, &moving)?;
        self.commit_candidate(candidate)
    }

    pub fn set_column_widths(
        &mut self,
        slide_index: usize,
        element_index: usize,
        left: u8,
        right: u8,
    ) -> Result<(), DocumentError> {
        if !(1..=99).contains(&left) || !(1..=99).contains(&right) || left + right != 100 {
            return Err(DocumentError::UnsafeValue(
                "column widths must each be 1..99 and add up to 100".into(),
            ));
        }
        let parsed = self.editable_document()?;
        let slide = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let node = slide
            .children()
            .and_then(|children| children.nodes().get(element_index))
            .filter(|node| node.name().value() == "columns")
            .ok_or_else(|| DocumentError::UnsafeValue("selected element is not columns".into()))?;
        let columns = node.children().ok_or(DocumentError::InvalidDocument)?;
        let mut edits = Vec::new();
        for (column, value) in columns.nodes().iter().zip([left, right]) {
            let entry = column
                .entries()
                .iter()
                .find(|entry| entry.name().is_some_and(|name| name.value() == "width"))
                .ok_or(DocumentError::InvalidDocument)?;
            edits.push((entry_value_range(&self.source, entry)?, value.to_string()));
        }
        edits.sort_by_key(|((start, _), _)| std::cmp::Reverse(*start));
        let mut candidate = self.source.clone();
        for ((start, end), replacement) in edits {
            candidate.replace_range(start..end, &replacement);
        }
        self.commit_candidate(candidate)
    }

    fn insert_code_language(
        &mut self,
        node: &KdlNode,
        language: &str,
    ) -> Result<(), DocumentError> {
        let insert_at = node_header_end(&self.source, node)?;
        let encoded = encode_kdl_string(language);
        let mut candidate = self.source.clone();
        candidate.insert_str(insert_at, &format!(" language={encoded}"));
        self.commit_candidate(candidate)
    }

    pub fn add_slide(&mut self) -> Result<usize, DocumentError> {
        let parsed = self.editable_document()?;
        let root = parsed
            .nodes()
            .first()
            .ok_or(DocumentError::InvalidDocument)?;
        let slide_index = self.model.as_ref().map_or(0, |model| model.slides.len());
        self.append_child(root, "slide {}")?;
        Ok(slide_index)
    }

    pub fn remove_slide(&mut self, slide_index: usize) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let node = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        self.remove_node(node)
    }

    /// Moves a slide while copying its original source slice byte-for-byte.
    /// Standalone comments remain in place; a same-line trailing comment moves
    /// with its slide. Inline siblings use their parser spans; insertion adds
    /// a node separator without changing quoted string contents.
    pub fn move_slide(&mut self, from: usize, to: usize) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let root = parsed
            .nodes()
            .first()
            .ok_or(DocumentError::InvalidDocument)?;
        let slides = root
            .children()
            .ok_or(DocumentError::InvalidDocument)?
            .nodes()
            .iter()
            .filter(|node| node.name().value() == "slide")
            .collect::<Vec<_>>();
        if from >= slides.len() || to >= slides.len() {
            return Err(DocumentError::UnsafeValue(
                "slide position is outside the presentation".into(),
            ));
        }
        if from == to {
            return Ok(());
        }

        let (start, end) = node_line_range(&self.source, slides[from])?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");

        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let root = reparsed
            .nodes()
            .first()
            .ok_or(DocumentError::InvalidDocument)?;
        // Metadata is always the first child of a valid presentation.
        insert_moved_source_at(&mut candidate, root, to + 1, &moving)?;
        self.commit_candidate(candidate)
    }

    /// Moves a top-level element on a slide using its original source bytes.
    /// Comment ownership follows the same rules as [`Self::move_slide`].
    pub fn move_element(
        &mut self,
        slide_index: usize,
        from: usize,
        to: usize,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let slide = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let elements = slide.children().ok_or(DocumentError::InvalidDocument)?;
        let nodes = elements.nodes();
        if from >= nodes.len() || to >= nodes.len() {
            return Err(DocumentError::UnsafeValue(
                "element position is outside the slide".into(),
            ));
        }
        if from == to {
            return Ok(());
        }

        let (start, end) = node_line_range(&self.source, &nodes[from])?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");

        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let slide = slide_node(&reparsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        insert_moved_source_at(&mut candidate, slide, to, &moving)?;
        self.commit_candidate(candidate)
    }

    pub fn move_element_to_column(
        &mut self,
        slide_index: usize,
        element_index: usize,
        columns_index: usize,
        column_index: usize,
    ) -> Result<(usize, usize), DocumentError> {
        self.move_element_to_column_at(
            slide_index,
            element_index,
            columns_index,
            column_index,
            usize::MAX,
        )
    }

    pub fn move_element_to_column_at(
        &mut self,
        slide_index: usize,
        element_index: usize,
        columns_index: usize,
        column_index: usize,
        destination_index: usize,
    ) -> Result<(usize, usize), DocumentError> {
        let parsed = self.editable_document()?;
        let slide = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let elements = slide.children().ok_or(DocumentError::InvalidDocument)?;
        let element = elements
            .nodes()
            .get(element_index)
            .ok_or(DocumentError::InvalidDocument)?;
        if element.name().value() == "columns" {
            return Err(DocumentError::UnsafeValue(
                "columns cannot be nested inside another column".into(),
            ));
        }
        let (start, end) = node_line_range(&self.source, element)?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");
        let target_columns_index = if element_index < columns_index {
            columns_index.saturating_sub(1)
        } else {
            columns_index
        };
        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let column = column_node(&reparsed, slide_index, target_columns_index, column_index)
            .ok_or(DocumentError::InvalidDocument)?;
        let moving =
            reindent_moved_source(&self.source, element, start, &moving, &candidate, column)?;
        let count = column
            .children()
            .map_or(0, |children| children.nodes().len());
        let destination_index = destination_index.min(count);
        insert_moved_source_at(&mut candidate, column, destination_index, &moving)?;
        self.commit_candidate(candidate)?;
        Ok((target_columns_index, destination_index))
    }

    pub fn move_column_element_to_slide(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        column_index: usize,
        element_index: usize,
    ) -> Result<usize, DocumentError> {
        self.move_column_element_to_slide_at(
            slide_index,
            columns_index,
            column_index,
            element_index,
            usize::MAX,
        )
    }

    pub fn move_column_element_to_slide_at(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        column_index: usize,
        element_index: usize,
        destination_index: usize,
    ) -> Result<usize, DocumentError> {
        let parsed = self.editable_document()?;
        let column = column_node(&parsed, slide_index, columns_index, column_index)
            .ok_or(DocumentError::InvalidDocument)?;
        let element = column
            .children()
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        if element.name().value() == "columns" {
            return Err(DocumentError::UnsafeValue(
                "columns cannot be nested inside another column".into(),
            ));
        }
        let slide_element_count = slide_node(&parsed, slide_index)
            .and_then(|slide| slide.children())
            .map_or(0, |children| children.nodes().len());
        let (start, end) = node_line_range(&self.source, element)?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");
        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let slide = slide_node(&reparsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let moving =
            reindent_moved_source(&self.source, element, start, &moving, &candidate, slide)?;
        let destination_index = destination_index.min(slide_element_count);
        insert_moved_source_at(&mut candidate, slide, destination_index, &moving)?;
        self.commit_candidate(candidate)?;
        Ok(destination_index)
    }

    pub fn move_column_element_to_column(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        source_column: usize,
        element_index: usize,
        target_column: usize,
    ) -> Result<usize, DocumentError> {
        self.move_column_element_to_column_at(
            slide_index,
            columns_index,
            source_column,
            element_index,
            target_column,
            usize::MAX,
        )
    }

    pub fn move_column_element_to_column_at(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        source_column: usize,
        element_index: usize,
        target_column: usize,
        destination_index: usize,
    ) -> Result<usize, DocumentError> {
        if source_column == target_column {
            let count = self
                .model()
                .and_then(|model| model.slides.get(slide_index))
                .and_then(|slide| slide.elements.get(columns_index))
                .and_then(|element| match element {
                    Element::Columns { left, right, .. } => Some(if source_column == 0 {
                        left.len()
                    } else {
                        right.len()
                    }),
                    _ => None,
                })
                .ok_or(DocumentError::InvalidDocument)?;
            let to = if destination_index == usize::MAX {
                count.saturating_sub(1)
            } else {
                destination_index.min(count.saturating_sub(1))
            };
            return self
                .move_column_element(slide_index, columns_index, source_column, element_index, to)
                .map(|()| to);
        }
        let parsed = self.editable_document()?;
        let source = column_node(&parsed, slide_index, columns_index, source_column)
            .ok_or(DocumentError::InvalidDocument)?;
        let element = source
            .children()
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        if element.name().value() == "columns" {
            return Err(DocumentError::UnsafeValue(
                "columns cannot be nested inside another column".into(),
            ));
        }
        let (start, end) = node_line_range(&self.source, element)?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");
        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let target = column_node(&reparsed, slide_index, columns_index, target_column)
            .ok_or(DocumentError::InvalidDocument)?;
        let moving =
            reindent_moved_source(&self.source, element, start, &moving, &candidate, target)?;
        let count = target
            .children()
            .map_or(0, |children| children.nodes().len());
        let destination_index = destination_index.min(count);
        insert_moved_source_at(&mut candidate, target, destination_index, &moving)?;
        self.commit_candidate(candidate)?;
        Ok(destination_index)
    }

    pub fn move_column_element_between_columns_at(
        &mut self,
        slide_index: usize,
        source_columns_index: usize,
        source_column: usize,
        element_index: usize,
        target_columns_index: usize,
        target_column: usize,
        destination_index: usize,
    ) -> Result<usize, DocumentError> {
        if source_columns_index == target_columns_index {
            return self.move_column_element_to_column_at(
                slide_index,
                source_columns_index,
                source_column,
                element_index,
                target_column,
                destination_index,
            );
        }
        let parsed = self.editable_document()?;
        let source = column_node(&parsed, slide_index, source_columns_index, source_column)
            .ok_or(DocumentError::InvalidDocument)?;
        let element = source
            .children()
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        if element.name().value() == "columns" {
            return Err(DocumentError::UnsafeValue(
                "columns cannot be nested inside another column".into(),
            ));
        }
        let (start, end) = node_line_range(&self.source, element)?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");
        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let target = column_node(&reparsed, slide_index, target_columns_index, target_column)
            .ok_or(DocumentError::InvalidDocument)?;
        let moving =
            reindent_moved_source(&self.source, element, start, &moving, &candidate, target)?;
        let count = target
            .children()
            .map_or(0, |children| children.nodes().len());
        let destination_index = destination_index.min(count);
        insert_moved_source_at(&mut candidate, target, destination_index, &moving)?;
        self.commit_candidate(candidate)?;
        Ok(destination_index)
    }

    pub fn add_element(
        &mut self,
        slide_index: usize,
        kind: ElementKind,
    ) -> Result<usize, DocumentError> {
        let parsed = self.editable_document()?;
        let slide = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let template = match kind {
            ElementKind::Heading => "heading \"\"",
            ElementKind::Text => "text \"\"",
            ElementKind::Bullets => "bullets { item \"\" }",
            ElementKind::Code => "code \"\"",
            ElementKind::Image => "image \"assets/image.png\"",
            ElementKind::Columns => "columns { column width=50; column width=50 }",
        };
        let index = slide
            .children()
            .map_or(0, |children| children.nodes().len());
        self.append_child(slide, template)?;
        Ok(index)
    }

    pub fn add_column_element(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        column_index: usize,
        kind: ElementKind,
    ) -> Result<usize, DocumentError> {
        let parsed = self.editable_document()?;
        let column = column_node(&parsed, slide_index, columns_index, column_index)
            .ok_or(DocumentError::InvalidDocument)?;
        let template = match kind {
            ElementKind::Heading => "heading \"\"",
            ElementKind::Text => "text \"\"",
            ElementKind::Bullets => "bullets { item \"\" }",
            ElementKind::Code => "code \"\"",
            ElementKind::Image => "image \"assets/image.png\"",
            ElementKind::Columns => {
                return Err(DocumentError::UnsafeValue(
                    "columns cannot be nested inside a column".into(),
                ));
            }
        };
        let index = column
            .children()
            .map_or(0, |children| children.nodes().len());
        self.append_child(column, template)?;
        Ok(index)
    }

    pub fn set_column_element_field(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        column_index: usize,
        element_index: usize,
        field: ElementField,
        value: &str,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let node = column_node(&parsed, slide_index, columns_index, column_index)
            .and_then(|column| column.children())
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        let entry = match field {
            ElementField::Text if matches!(node.name().value(), "heading" | "text" | "code") => {
                node.entries().iter().find(|entry| entry.name().is_none())
            }
            ElementField::ImagePath if node.name().value() == "image" => {
                node.entries().iter().find(|entry| entry.name().is_none())
            }
            ElementField::CodeLanguage if node.name().value() == "code" => node
                .entries()
                .iter()
                .find(|entry| entry.name().is_some_and(|name| name.value() == "language")),
            ElementField::Bullet(index) if node.name().value() == "bullets" => node
                .children()
                .and_then(|children| children.nodes().get(index))
                .and_then(|item| item.entries().first()),
            ElementField::Caption if node.name().value() == "image" => node
                .children()
                .and_then(|children| {
                    children
                        .nodes()
                        .iter()
                        .find(|child| child.name().value() == "caption")
                })
                .and_then(|caption| caption.entries().first()),
            _ => None,
        };
        if let Some(entry) = entry {
            return self.replace_entry_string(entry, value);
        }
        if field == ElementField::CodeLanguage && node.name().value() == "code" {
            return self.insert_code_language(node, value);
        }
        if field == ElementField::Caption && node.name().value() == "image" {
            return self.append_child(node, &format!("caption {}", encode_kdl_string(value)));
        }
        Err(DocumentError::UnsafeValue(
            "selected element has no matching text field".into(),
        ))
    }

    pub fn set_column_bullets_text(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        column_index: usize,
        element_index: usize,
        text: &str,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let node = column_node(&parsed, slide_index, columns_index, column_index)
            .and_then(|column| column.children())
            .and_then(|children| children.nodes().get(element_index))
            .filter(|node| node.name().value() == "bullets")
            .ok_or_else(|| DocumentError::UnsafeValue("selected element is not bullets".into()))?;
        let requested = text.split('\n').collect::<Vec<_>>();
        let existing = node
            .children()
            .map(|children| children.nodes())
            .unwrap_or(&[]);
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        for (item, value) in existing.iter().zip(requested.iter()) {
            let entry = item
                .entries()
                .first()
                .ok_or(DocumentError::InvalidDocument)?;
            let (start, end) = entry_string_range(&self.source, entry)?;
            edits.push((start, end, encode_kdl_string(value)));
        }
        for item in existing.iter().skip(requested.len()) {
            edits.push((
                node_source_start(&self.source, item),
                item.span().offset() + item.span().len(),
                String::new(),
            ));
        }
        if requested.len() > existing.len() {
            let additions = requested
                .iter()
                .skip(existing.len())
                .map(|value| format!("item {}", encode_kdl_string(value)))
                .collect::<Vec<_>>();
            edits.push(child_insertion(&self.source, node, &additions.join("\n"))?);
        }
        edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
        let mut candidate = self.source.clone();
        for (start, end, replacement) in edits {
            if end > candidate.len()
                || start > end
                || !candidate.is_char_boundary(start)
                || !candidate.is_char_boundary(end)
            {
                return Err(DocumentError::UnsafeValue(
                    "invalid source range while editing bullets".into(),
                ));
            }
            candidate.replace_range(start..end, &replacement);
        }
        self.commit_candidate(candidate)
    }

    pub fn remove_column_element(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        column_index: usize,
        element_index: usize,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let node = column_node(&parsed, slide_index, columns_index, column_index)
            .and_then(|column| column.children())
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        self.remove_node(node)
    }

    pub fn move_column_element(
        &mut self,
        slide_index: usize,
        columns_index: usize,
        column_index: usize,
        from: usize,
        to: usize,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let column = column_node(&parsed, slide_index, columns_index, column_index)
            .ok_or(DocumentError::InvalidDocument)?;
        let nodes = column
            .children()
            .map(|children| children.nodes())
            .unwrap_or(&[]);
        if from >= nodes.len() || to >= nodes.len() {
            return Err(DocumentError::UnsafeValue(
                "element position is outside the column".into(),
            ));
        }
        if from == to {
            return Ok(());
        }
        let (start, end) = node_line_range(&self.source, &nodes[from])?;
        let moving = self.source[start..end].to_owned();
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");
        let reparsed =
            KdlDocument::parse_v2(&candidate).map_err(|_| DocumentError::InvalidDocument)?;
        let column = column_node(&reparsed, slide_index, columns_index, column_index)
            .ok_or(DocumentError::InvalidDocument)?;
        insert_moved_source_at(&mut candidate, column, to, &moving)?;
        self.commit_candidate(candidate)
    }

    pub fn remove_element(
        &mut self,
        slide_index: usize,
        element_index: usize,
    ) -> Result<(), DocumentError> {
        let parsed = self.editable_document()?;
        let slide = slide_node(&parsed, slide_index).ok_or(DocumentError::InvalidDocument)?;
        let node = slide
            .children()
            .and_then(|children| children.nodes().get(element_index))
            .ok_or(DocumentError::InvalidDocument)?;
        self.remove_node(node)
    }

    fn append_child(&mut self, parent: &KdlNode, child_source: &str) -> Result<(), DocumentError> {
        let (start, end, replacement) = child_insertion(&self.source, parent, child_source)?;
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, &replacement);
        self.commit_candidate(candidate)
    }

    fn remove_node(&mut self, node: &KdlNode) -> Result<(), DocumentError> {
        let (start, end) = node_line_range(&self.source, node).unwrap_or_else(|_| {
            (
                node_source_start(&self.source, node),
                node.span().offset().saturating_add(node.span().len()),
            )
        });
        if end > self.source.len()
            || !self.source.is_char_boundary(start)
            || !self.source.is_char_boundary(end)
        {
            return Err(DocumentError::UnsafeValue(
                "KDL parser returned an invalid node span".into(),
            ));
        }
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, "");
        self.commit_candidate(candidate)
    }

    fn editable_document(&self) -> Result<KdlDocument, DocumentError> {
        if !self.can_edit() {
            return Err(DocumentError::InvalidDocument);
        }
        KdlDocument::parse_v2(&self.source).map_err(|_| DocumentError::InvalidDocument)
    }

    fn replace_entry_string(&mut self, entry: &KdlEntry, value: &str) -> Result<(), DocumentError> {
        let (start, end) = entry_string_range(&self.source, entry)?;
        let encoded = encode_kdl_string(value);
        let mut candidate = self.source.clone();
        candidate.replace_range(start..end, &encoded);
        self.commit_candidate(candidate)
    }

    fn commit_candidate(&mut self, candidate: String) -> Result<(), DocumentError> {
        let (model, diagnostics) = parse_and_validate(&candidate, self.path.as_deref());
        if model.is_none() || !diagnostics.is_empty() {
            return Err(DocumentError::InvalidDocument);
        }
        if candidate == self.source {
            return Ok(());
        }
        let description = describe_model_change(self.model.as_ref(), model.as_ref());
        self.undo
            .push(std::mem::replace(&mut self.source, candidate));
        self.undo_labels.push(description);
        if self.undo.len() > 1_000 {
            self.undo.remove(0);
            self.undo_labels.remove(0);
        }
        self.redo.clear();
        self.redo_labels.clear();
        self.model = model;
        self.diagnostics = diagnostics;
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some((source, description)) = self.undo.pop().zip(self.undo_labels.pop()) else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.source, source));
        self.redo_labels.push(description);
        self.refresh_model();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some((source, description)) = self.redo.pop().zip(self.redo_labels.pop()) else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.source, source));
        self.undo_labels.push(description);
        self.refresh_model();
        true
    }

    pub fn check_external_change(&self) -> Result<(), DocumentError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if fs::read(path)? != self.saved_source.as_bytes() {
            return Err(DocumentError::ExternalChange);
        }
        Ok(())
    }

    fn refresh_model(&mut self) {
        (self.model, self.diagnostics) = parse_and_validate(&self.source, self.path.as_deref());
    }

    pub fn save(&mut self) -> Result<(), DocumentError> {
        self.save_before_replace(|| {})
    }

    // Private seam for deterministic tests of changes during the temporary write.
    fn save_before_replace(&mut self, before_replace: impl FnOnce()) -> Result<(), DocumentError> {
        let path = self.path.clone().ok_or(DocumentError::NoPath)?;
        if !self.can_edit() {
            return Err(DocumentError::InvalidDocument);
        }
        let current_disk = fs::read(&path)?;
        if current_disk != self.saved_source.as_bytes() {
            return Err(DocumentError::ExternalChange);
        }

        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(self.source.as_bytes())?;
        temp.as_file().sync_all()?;
        if let Ok(metadata) = fs::metadata(&path) {
            temp.as_file().set_permissions(metadata.permissions())?;
        }
        before_replace();
        // Recheck just before the atomic rename so an editor update during the
        // temporary-file write does not get silently overwritten.
        if fs::read(&path)? != self.saved_source.as_bytes() {
            return Err(DocumentError::ExternalChange);
        }
        temp.persist(&path)
            .map_err(|error| DocumentError::Io(error.error))?;
        if let Ok(directory) = fs::File::open(parent) {
            let _ = directory.sync_all();
        }
        self.saved_source.clone_from(&self.source);
        Ok(())
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<(), DocumentError> {
        self.save_as_inner(path.as_ref(), false)
    }

    /// Save to a destination the user explicitly approved replacing in the
    /// native Save panel.
    pub fn save_as_overwriting(&mut self, path: impl AsRef<Path>) -> Result<(), DocumentError> {
        self.save_as_inner(path.as_ref(), true)
    }

    fn save_as_inner(
        &mut self,
        path: &Path,
        overwrite_confirmed: bool,
    ) -> Result<(), DocumentError> {
        if !self.can_edit() {
            return Err(DocumentError::InvalidDocument);
        }
        let path = path.to_path_buf();
        let previous_destination = match fs::read(&path) {
            Ok(bytes) if overwrite_confirmed => Some(bytes),
            Ok(_) => return Err(DocumentError::DestinationExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(DocumentError::Io(error)),
        };
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let rebased_source = rebase_image_paths(&self.source, self.asset_base(), &parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(&parent)?;
        temp.write_all(rebased_source.as_bytes())?;
        temp.as_file().sync_all()?;
        if let Some(previous_destination) = previous_destination {
            let metadata = fs::metadata(&path)?;
            if fs::read(&path)? != previous_destination {
                return Err(DocumentError::ExternalChange);
            }
            temp.as_file().set_permissions(metadata.permissions())?;
            temp.persist(&path)
                .map_err(|error| DocumentError::Io(error.error))?;
        } else {
            temp.persist_noclobber(&path).map_err(|error| {
                if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                    DocumentError::DestinationExists
                } else {
                    DocumentError::Io(error.error)
                }
            })?;
        }
        if let Ok(directory) = fs::File::open(&parent) {
            let _ = directory.sync_all();
        }
        self.source = rebased_source;
        self.path = Some(path);
        self.asset_base = Some(parent);
        self.saved_source.clone_from(&self.source);
        self.undo.clear();
        self.redo.clear();
        self.undo_labels.clear();
        self.redo_labels.clear();
        self.refresh_model();
        Ok(())
    }
}

fn describe_model_change(
    before: Option<&PresentationModel>,
    after: Option<&PresentationModel>,
) -> String {
    let (Some(before), Some(after)) = (before, after) else {
        return "Presentationを編集".into();
    };
    if before.title != after.title {
        return "タイトルを編集".into();
    }
    if before.slides.len() != after.slides.len() {
        return if before.slides.len() < after.slides.len() {
            "Slideを追加"
        } else {
            "Slideを削除"
        }
        .into();
    }
    if before.slides != after.slides && same_members(&before.slides, &after.slides) {
        return "Slideを並べ替え".into();
    }
    for (before_slide, after_slide) in before.slides.iter().zip(&after.slides) {
        if before_slide.id != after_slide.id {
            return "Slideを編集".into();
        }
        let before_positions = leaf_element_positions(before_slide);
        let after_positions = leaf_element_positions(after_slide);
        if before_positions != after_positions
            && same_members(
                &before_positions
                    .iter()
                    .map(|(_, element)| element.clone())
                    .collect::<Vec<_>>(),
                &after_positions
                    .iter()
                    .map(|(_, element)| element.clone())
                    .collect::<Vec<_>>(),
            )
        {
            return "Elementを移動".into();
        }
        if before_slide.elements.len() != after_slide.elements.len() {
            return if before_slide.elements.len() < after_slide.elements.len() {
                "Elementを追加"
            } else {
                "Elementを削除"
            }
            .into();
        }
        if before_slide.elements != after_slide.elements
            && same_members(&before_slide.elements, &after_slide.elements)
        {
            return "Elementを並べ替え".into();
        }
        for (before_element, after_element) in
            before_slide.elements.iter().zip(&after_slide.elements)
        {
            if before_element != after_element {
                return describe_element_change(before_element, after_element).into();
            }
        }
    }
    "KDLソースを編集".into()
}

fn describe_element_change(before: &Element, after: &Element) -> &'static str {
    match (before, after) {
        (Element::Heading(_), Element::Heading(_)) | (Element::Text(_), Element::Text(_)) => {
            "テキストを編集"
        }
        (Element::Bullets(_), Element::Bullets(_)) => "箇条書きを編集",
        (
            Element::Code {
                text: before_text,
                language: before_language,
            },
            Element::Code {
                text: after_text,
                language: after_language,
            },
        ) => {
            if before_text == after_text && before_language != after_language {
                "コード言語を編集"
            } else {
                "コードを編集"
            }
        }
        (
            Element::Image {
                path: before_path,
                caption: before_caption,
            },
            Element::Image {
                path: after_path,
                caption: after_caption,
            },
        ) => match (before_path != after_path, before_caption != after_caption) {
            (true, false) => "画像参照を編集",
            (false, true) => "画像Captionを編集",
            _ => "画像を編集",
        },
        (
            Element::Columns {
                left_width: before_width,
                left: before_left,
                right: before_right,
                ..
            },
            Element::Columns {
                left_width: after_width,
                left: after_left,
                right: after_right,
                ..
            },
        ) => {
            if before_width != after_width {
                "列幅を変更"
            } else if before_left.len() + before_right.len() != after_left.len() + after_right.len()
            {
                if before_left.len() + before_right.len() < after_left.len() + after_right.len() {
                    "列内Elementを追加"
                } else {
                    "列内Elementを削除"
                }
            } else {
                "列内Elementを編集"
            }
        }
        _ => "Elementを編集",
    }
}

fn leaf_element_positions(slide: &Slide) -> Vec<(Vec<usize>, Element)> {
    let mut output = Vec::new();
    for (index, element) in slide.elements.iter().enumerate() {
        collect_leaf_element_positions(element, &mut vec![index], &mut output);
    }
    output
}

fn collect_leaf_element_positions(
    element: &Element,
    path: &mut Vec<usize>,
    output: &mut Vec<(Vec<usize>, Element)>,
) {
    match element {
        Element::Columns { left, right, .. } => {
            for (column, elements) in [(0, left), (1, right)] {
                path.push(column);
                for (index, nested) in elements.iter().enumerate() {
                    path.push(index);
                    collect_leaf_element_positions(nested, path, output);
                    path.pop();
                }
                path.pop();
            }
        }
        _ => output.push((path.clone(), element.clone())),
    }
}

fn same_members<T: Clone + PartialEq>(before: &[T], after: &[T]) -> bool {
    if before.len() != after.len() {
        return false;
    }
    let mut remaining = after.to_vec();
    for item in before {
        let Some(index) = remaining.iter().position(|candidate| candidate == item) else {
            return false;
        };
        remaining.remove(index);
    }
    true
}

fn push_asset_diagnostic(
    diagnostics: &mut Vec<AssetDiagnostic>,
    base: &Path,
    path: &str,
    slide: &Slide,
    slide_index: usize,
    element_index: usize,
    column_index: Option<usize>,
    nested_element_index: Option<usize>,
) {
    if let Some(problem) = inspect_asset_file(&base.join(path)) {
        diagnostics.push(AssetDiagnostic {
            path: path.to_owned(),
            problem,
            slide_index,
            slide_id: slide.id.clone(),
            element_index,
            column_index,
            nested_element_index,
        });
    }
}

fn rebase_image_paths(
    source: &str,
    old_base: &Path,
    new_base: &Path,
) -> Result<String, DocumentError> {
    let parsed = KdlDocument::parse_v2(source).map_err(|_| DocumentError::InvalidDocument)?;
    let old_base = absolute_lexical_path(old_base)?;
    let new_base = absolute_lexical_path(new_base)?;
    let mut entries = Vec::new();
    for node in parsed.nodes() {
        collect_image_entries(node, &mut entries);
    }

    let mut edits = Vec::new();
    for entry in entries {
        let Some(path) = entry.value().as_string() else {
            continue;
        };
        let asset_path = absolute_lexical_path(&old_base.join(path))?;
        let relative = relative_path(&new_base, &asset_path).ok_or_else(|| {
            DocumentError::UnsafeValue(
                "image asset cannot be referenced from the save destination".into(),
            )
        })?;
        let encoded = encode_kdl_string(&relative.to_string_lossy());
        edits.push((entry_string_range(source, entry)?, encoded));
    }

    edits.sort_by_key(|((start, _), _)| std::cmp::Reverse(*start));
    let mut rebased = source.to_owned();
    for ((start, end), replacement) in edits {
        rebased.replace_range(start..end, &replacement);
    }
    Ok(rebased)
}

fn collect_image_entries<'a>(node: &'a KdlNode, entries: &mut Vec<&'a KdlEntry>) {
    if node.name().value() == "image" {
        if let Some(entry) = node.entries().iter().find(|entry| entry.name().is_none()) {
            entries.push(entry);
        }
    }
    if let Some(children) = node.children() {
        for child in children.nodes() {
            collect_image_entries(child, entries);
        }
    }
}

fn absolute_lexical_path(path: &Path) -> Result<PathBuf, DocumentError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|last| last != std::path::Component::RootDir)
                {
                    normalized.pop();
                }
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return None;
    }
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    Some(relative)
}

fn parse_and_validate(
    source: &str,
    path: Option<&Path>,
) -> (Option<PresentationModel>, Vec<Diagnostic>) {
    match KdlDocument::parse_v2(source) {
        Ok(document) => validate_document(source, path, &document),
        Err(error) => {
            let file_name = path
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned());
            let diagnostics = error
                .diagnostics
                .iter()
                .map(|detail| {
                    diagnostic(
                        source,
                        DiagnosticKind::Syntax,
                        detail
                            .message
                            .clone()
                            .unwrap_or_else(|| "invalid KDL syntax".into()),
                        detail.span.offset(),
                        detail.span.len(),
                        file_name.clone(),
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                })
                .collect();
            (None, diagnostics)
        }
    }
}

fn validate_document(
    source: &str,
    path: Option<&Path>,
    document: &KdlDocument,
) -> (Option<PresentationModel>, Vec<Diagnostic>) {
    let mut validator = Validator {
        source,
        file_name: path
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned()),
        diagnostics: Vec::new(),
        slide_index: None,
        slide_id: None,
        element_index: None,
        column_index: None,
        nested_element_index: None,
    };
    if document.nodes().len() != 1 {
        validator.add(
            "document must contain exactly one presentation node",
            0,
            source.len(),
        );
    }
    let root = document.nodes().first();
    let Some(root) = root else {
        return (None, validator.diagnostics);
    };
    if root.name().value() != "presentation" {
        validator.node_error(root, "unknown root node; expected presentation");
        return (None, validator.diagnostics);
    }
    validator.check_node_header(root, &[], 0, "presentation");
    let Some(root_children) = root.children() else {
        validator.node_error(root, "presentation must contain metadata");
        return (None, validator.diagnostics);
    };
    let children = root_children.nodes();
    if children
        .first()
        .is_none_or(|node| node.name().value() != "metadata")
    {
        validator.add(
            "metadata must be the first presentation child",
            root.span().offset(),
            root.span().len(),
        );
    }
    let metadata = children
        .iter()
        .filter(|node| node.name().value() == "metadata")
        .collect::<Vec<_>>();
    if metadata.len() != 1 {
        validator.add(
            "presentation must contain exactly one metadata node",
            root.span().offset(),
            root.span().len(),
        );
    }
    let mut title = String::new();
    if let Some(metadata) = metadata.first() {
        validator.check_node_header(metadata, &[], 0, "metadata");
        if let Some(children) = metadata.children() {
            if children.nodes().len() != 1 || children.nodes()[0].name().value() != "title" {
                validator.node_error(metadata, "metadata must contain exactly one title node");
            }
            if let Some(title_node) = children
                .nodes()
                .iter()
                .find(|node| node.name().value() == "title")
            {
                validator.check_string_node(title_node, "title", false);
                title = positional_string(title_node).unwrap_or_default();
                if title.is_empty() {
                    validator.node_error(title_node, "title must not be empty");
                }
            }
        } else {
            validator.node_error(metadata, "metadata must contain a title node");
        }
    }
    let mut slides = Vec::new();
    let mut slide_ids = HashSet::new();
    for (index, node) in children.iter().enumerate().skip(1) {
        if node.name().value() != "slide" {
            validator.node_error(node, "unknown presentation child; expected slide");
            continue;
        }
        validator.slide_index = Some(slides.len());
        validator.slide_id = property_string(node, "id");
        validator.check_node_header(node, &["id"], 0, "slide");
        validator.check_string_attribute(node, "id");
        if let Some(id) = property_string(node, "id") {
            if !slide_ids.insert(id.clone()) {
                validator.node_error(node, "duplicate slide id");
            }
        }
        let elements = node
            .children()
            .map(|children| validator.parse_elements(children.nodes(), true, None))
            .unwrap_or_default();
        slides.push(Slide {
            id: property_string(node, "id"),
            elements,
        });
        let _ = index;
    }
    if validator.diagnostics.is_empty() {
        (Some(PresentationModel { title, slides }), Vec::new())
    } else {
        (None, validator.diagnostics)
    }
}

struct Validator<'a> {
    source: &'a str,
    file_name: Option<String>,
    diagnostics: Vec<Diagnostic>,
    slide_index: Option<usize>,
    slide_id: Option<String>,
    element_index: Option<usize>,
    column_index: Option<usize>,
    nested_element_index: Option<usize>,
}

impl Validator<'_> {
    fn add(&mut self, message: impl Into<String>, offset: usize, length: usize) {
        self.diagnostics.push(diagnostic(
            self.source,
            DiagnosticKind::Schema,
            message.into(),
            offset,
            length,
            self.file_name.clone(),
            self.slide_index,
            self.slide_id.clone(),
            self.element_index,
            self.column_index,
            self.nested_element_index,
        ));
    }

    fn node_error(&mut self, node: &KdlNode, message: impl Into<String>) {
        self.add(message, node.span().offset(), node.span().len());
    }

    fn check_node_header(
        &mut self,
        node: &KdlNode,
        allowed_attrs: &[&str],
        positional_count: usize,
        expected_name: &str,
    ) {
        if node.ty().is_some() {
            self.node_error(
                node,
                "typed nodes are not supported by Presentation Schema v0.1",
            );
        }
        if node.name().value() != expected_name {
            self.node_error(node, format!("unknown node; expected {expected_name}"));
        }
        let positional = node
            .entries()
            .iter()
            .filter(|entry| entry.name().is_none())
            .count();
        if positional != positional_count {
            self.node_error(
                node,
                format!("{expected_name} expects {positional_count} positional value(s)"),
            );
        }
        let mut names = HashSet::new();
        for entry in node.entries() {
            if let Some(name) = entry.name() {
                if !names.insert(name.value()) {
                    self.add(
                        "duplicate attribute",
                        entry.span().offset(),
                        entry.span().len(),
                    );
                }
                if !allowed_attrs.contains(&name.value()) {
                    self.add(
                        format!("unknown attribute `{}`", name.value()),
                        entry.span().offset(),
                        entry.span().len(),
                    );
                }
                if entry.ty().is_some() {
                    self.add(
                        "typed values are not supported by Presentation Schema v0.1",
                        entry.span().offset(),
                        entry.span().len(),
                    );
                }
            } else if entry.ty().is_some() {
                self.add(
                    "typed values are not supported by Presentation Schema v0.1",
                    entry.span().offset(),
                    entry.span().len(),
                );
            }
        }
    }

    fn check_string_node(&mut self, node: &KdlNode, name: &str, allow_attrs: bool) {
        self.check_node_header(node, if allow_attrs { &["language"] } else { &[] }, 1, name);
        if positional_string(node).is_none() {
            self.node_error(node, format!("{name} must contain a string value"));
        }
        if node.children().is_some() {
            self.node_error(node, format!("{name} cannot contain child nodes"));
        }
    }

    fn check_string_attribute(&mut self, node: &KdlNode, name: &str) {
        if let Some(entry) = node
            .entries()
            .iter()
            .find(|entry| entry.name().is_some_and(|key| key.value() == name))
        {
            if entry.value().as_string().is_none() {
                self.add(
                    format!("attribute `{name}` must be a string"),
                    entry.span().offset(),
                    entry.span().len(),
                );
            }
        }
    }

    fn parse_elements(
        &mut self,
        nodes: &[KdlNode],
        allow_columns: bool,
        nested_context: Option<(usize, usize)>,
    ) -> Vec<Element> {
        let previous_context = (
            self.element_index,
            self.column_index,
            self.nested_element_index,
        );
        let mut elements = Vec::new();
        for (node_index, node) in nodes.iter().enumerate() {
            if let Some((parent_index, column_index)) = nested_context {
                self.element_index = Some(parent_index);
                self.column_index = Some(column_index);
                self.nested_element_index = Some(node_index);
            } else {
                self.element_index = Some(node_index);
                self.column_index = None;
                self.nested_element_index = None;
            }
            match node.name().value() {
                "heading" | "text" => {
                    self.check_string_node(node, node.name().value(), false);
                    let value = positional_string(node).unwrap_or_default();
                    elements.push(if node.name().value() == "heading" {
                        Element::Heading(value)
                    } else {
                        Element::Text(value)
                    });
                }
                "code" => {
                    self.check_string_node(node, "code", true);
                    self.check_string_attribute(node, "language");
                    let language = property_string(node, "language");
                    if let Some(value) = node
                        .entries()
                        .iter()
                        .find(|entry| entry.name().is_some_and(|name| name.value() == "language"))
                    {
                        if value.value().as_string().is_none() {
                            self.add(
                                "code language must be a string",
                                value.span().offset(),
                                value.span().len(),
                            );
                        }
                    }
                    elements.push(Element::Code {
                        text: positional_string(node).unwrap_or_default(),
                        language,
                    });
                }
                "bullets" => {
                    self.check_node_header(node, &[], 0, "bullets");
                    let mut items = Vec::new();
                    if let Some(children) = node.children() {
                        for item in children.nodes() {
                            if item.name().value() != "item" {
                                self.node_error(item, "unknown bullets child; expected item");
                            }
                            self.check_string_node(item, "item", false);
                            items.push(positional_string(item).unwrap_or_default());
                        }
                    }
                    elements.push(Element::Bullets(items));
                }
                "image" => {
                    self.check_node_header(node, &[], 1, "image");
                    let path = positional_string(node).unwrap_or_default();
                    if path.is_empty() || Path::new(&path).is_absolute() {
                        self.node_error(node, "image path must be a non-empty relative path");
                    }
                    let mut caption = None;
                    if let Some(children) = node.children() {
                        if children.nodes().len() > 1
                            || children
                                .nodes()
                                .iter()
                                .any(|child| child.name().value() != "caption")
                        {
                            self.node_error(node, "image may contain at most one caption child");
                        }
                        if let Some(child) = children.nodes().first() {
                            self.check_string_node(child, "caption", false);
                            caption = positional_string(child);
                        }
                    }
                    elements.push(Element::Image { path, caption });
                }
                "columns" if allow_columns => {
                    self.check_node_header(node, &[], 0, "columns");
                    let columns = node
                        .children()
                        .map(|children| children.nodes())
                        .unwrap_or(&[]);
                    if columns.len() != 2
                        || columns
                            .iter()
                            .any(|column| column.name().value() != "column")
                    {
                        self.node_error(node, "columns must contain exactly two column nodes");
                        continue;
                    }
                    let left_width = self.parse_column(&columns[0]);
                    let right_width = self.parse_column(&columns[1]);
                    if let (Some(left_width), Some(right_width)) = (left_width, right_width) {
                        if left_width + right_width != 100 {
                            self.node_error(node, "column widths must add up to 100");
                        }
                        let parent_index = node_index;
                        let left = columns[0]
                            .children()
                            .map(|children| {
                                self.parse_elements(
                                    children.nodes(),
                                    false,
                                    Some((parent_index, 0)),
                                )
                            })
                            .unwrap_or_default();
                        let right = columns[1]
                            .children()
                            .map(|children| {
                                self.parse_elements(
                                    children.nodes(),
                                    false,
                                    Some((parent_index, 1)),
                                )
                            })
                            .unwrap_or_default();
                        elements.push(Element::Columns {
                            left_width,
                            right_width,
                            left,
                            right,
                        });
                    }
                }
                "columns" => self.node_error(node, "columns cannot be nested inside a column"),
                _ => self.node_error(node, format!("unknown element `{}`", node.name().value())),
            }
        }
        (
            self.element_index,
            self.column_index,
            self.nested_element_index,
        ) = previous_context;
        elements
    }

    fn parse_column(&mut self, node: &KdlNode) -> Option<u8> {
        self.check_node_header(node, &["width"], 0, "column");
        let width = node
            .entries()
            .iter()
            .find(|entry| entry.name().is_some_and(|name| name.value() == "width"));
        let Some(width) = width else {
            self.node_error(node, "column requires width attribute");
            return None;
        };
        match width
            .value()
            .as_integer()
            .and_then(|value| u8::try_from(value).ok())
        {
            Some(value @ 1..=99) => Some(value),
            _ => {
                self.add(
                    "column width must be an integer from 1 to 99",
                    width.span().offset(),
                    width.span().len(),
                );
                None
            }
        }
    }
}

fn positional_string(node: &KdlNode) -> Option<String> {
    let entries = node
        .entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .collect::<Vec<_>>();
    (entries.len() == 1)
        .then(|| entries[0].value().as_string().map(str::to_owned))
        .flatten()
}

fn collect_image_paths<'a>(element: &'a Element, paths: &mut Vec<&'a str>) {
    match element {
        Element::Image { path, .. } => paths.push(path),
        Element::Columns { left, right, .. } => {
            for element in left.iter().chain(right) {
                collect_image_paths(element, paths);
            }
        }
        _ => {}
    }
}

fn inspect_asset_file(path: &Path) -> Option<AssetProblem> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Some(match error.kind() {
                std::io::ErrorKind::NotFound => AssetProblem::Missing,
                std::io::ErrorKind::PermissionDenied => AssetProblem::PermissionDenied,
                _ => AssetProblem::Corrupt,
            });
        }
    };
    let Some(format) = image::guess_format(&bytes).ok() else {
        return Some(AssetProblem::UnsupportedFormat);
    };
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP
    ) {
        return Some(AssetProblem::UnsupportedFormat);
    }
    if format == image::ImageFormat::Png && png_is_animated(&bytes) {
        return Some(AssetProblem::AnimatedImage);
    }
    if format == image::ImageFormat::WebP {
        match image::codecs::webp::WebPDecoder::new(Cursor::new(&bytes)) {
            Ok(decoder) if decoder.has_animation() => return Some(AssetProblem::AnimatedImage),
            Err(_) => return Some(AssetProblem::Corrupt),
            _ => {}
        }
    }
    image::load_from_memory_with_format(&bytes, format)
        .err()
        .map(|_| AssetProblem::Corrupt)
}

fn png_is_animated(bytes: &[u8]) -> bool {
    if bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return false;
    }
    let mut cursor = 8usize;
    while cursor + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let Some(chunk_end) = cursor
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
        else {
            return false;
        };
        if chunk_end > bytes.len() {
            return false;
        }
        if &bytes[cursor + 4..cursor + 8] == b"acTL" {
            return true;
        }
        cursor = chunk_end;
    }
    false
}

fn property_string(node: &KdlNode, key: &str) -> Option<String> {
    node.entries()
        .iter()
        .find(|entry| entry.name().is_some_and(|name| name.value() == key))
        .and_then(|entry| entry.value().as_string())
        .map(str::to_owned)
}

fn child_named<'a>(node: &'a KdlNode, name: &str, index: usize) -> Option<&'a KdlNode> {
    node.children()?
        .nodes()
        .iter()
        .filter(|child| child.name().value() == name)
        .nth(index)
}

fn slide_node(document: &KdlDocument, slide_index: usize) -> Option<&KdlNode> {
    document
        .nodes()
        .first()?
        .children()?
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "slide")
        .nth(slide_index)
}

fn column_node(
    document: &KdlDocument,
    slide_index: usize,
    columns_index: usize,
    column_index: usize,
) -> Option<&KdlNode> {
    let columns = slide_node(document, slide_index)?
        .children()?
        .nodes()
        .get(columns_index)?;
    if columns.name().value() != "columns" {
        return None;
    }
    columns
        .children()?
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "column")
        .nth(column_index)
}

fn child_insertion(
    source: &str,
    parent: &KdlNode,
    child_source: &str,
) -> Result<(usize, usize, String), DocumentError> {
    let parent_indent = line_indent(source, parent.name().span().offset());
    let child_indent = parent
        .children()
        .and_then(|children| children.nodes().first())
        .map(|child| line_indent(source, child.name().span().offset()))
        .filter(|indent| indent.len() > parent_indent.len())
        .unwrap_or_else(|| format!("{parent_indent}    "));
    let insertion_context = if parent.children().is_some() {
        node_children_close(source, parent)?
    } else {
        node_header_end(source, parent)?
    };
    let line_ending = line_ending_near(source, insertion_context);
    let indented_child = child_source.replace('\n', &format!("{line_ending}{child_indent}"));
    if parent.children().is_some() {
        let close = node_children_close(source, parent)?;
        let line_start = source[..close].rfind('\n').map_or(0, |index| index + 1);
        if source[line_start..close].chars().all(char::is_whitespace) {
            Ok((
                line_start,
                line_start,
                format!("{child_indent}{indented_child}{line_ending}"),
            ))
        } else {
            Ok((
                close,
                close,
                format!("{line_ending}{child_indent}{indented_child}{line_ending}{parent_indent}"),
            ))
        }
    } else {
        let insert_at = node_header_end(source, parent)?;
        Ok((
            insert_at,
            insert_at,
            format!(" {{{line_ending}{child_indent}{indented_child}{line_ending}{parent_indent}}}"),
        ))
    }
}

fn insert_moved_source_at(
    source: &mut String,
    parent: &KdlNode,
    destination_index: usize,
    moving: &str,
) -> Result<(), DocumentError> {
    if parent.children().is_none() {
        let insertion = node_header_end(source, parent)?;
        let line_ending = line_ending_near(source, insertion);
        let mut moving = normalize_line_endings_outside_strings(moving, line_ending);
        if !moving.ends_with('\n') {
            moving.push_str(line_ending);
        }
        let parent_indent = line_indent(source, parent.name().span().offset());
        source.insert_str(
            insertion,
            &format!(" {{{line_ending}{moving}{parent_indent}}}"),
        );
        return Ok(());
    }
    let children = parent.children().ok_or(DocumentError::InvalidDocument)?;
    if let Some(target) = children.nodes().get(destination_index) {
        let insertion = node_source_start(source, target);
        let line_ending = line_ending_near(source, insertion);
        let mut moving = normalize_line_endings_outside_strings(moving, line_ending);
        if !moving.ends_with('\n') {
            moving.push_str(line_ending);
        }
        source.insert_str(insertion, &moving);
        return Ok(());
    }

    let close = node_children_close(source, parent)?;
    let line_start = source[..close].rfind('\n').map_or(0, |index| index + 1);
    let line_ending = line_ending_near(
        source,
        if line_start < close {
            line_start
        } else {
            close
        },
    );
    let mut moving = normalize_line_endings_outside_strings(moving, line_ending);
    if !moving.ends_with('\n') {
        moving.push_str(line_ending);
    }
    if source[line_start..close].chars().all(char::is_whitespace) {
        source.insert_str(line_start, &moving);
    } else {
        let parent_indent = line_indent(source, parent.name().span().offset());
        source.insert_str(close, &format!("{line_ending}{moving}{parent_indent}"));
    }
    Ok(())
}

fn reindent_moved_source(
    original_source: &str,
    moved_node: &KdlNode,
    moved_start: usize,
    moving: &str,
    destination_source: &str,
    destination_parent: &KdlNode,
) -> Result<String, DocumentError> {
    let parent_indent = line_indent(
        destination_source,
        destination_parent.name().span().offset(),
    );
    let child_indent = destination_parent
        .children()
        .and_then(|children| children.nodes().first())
        .map(|child| line_indent(destination_source, child.name().span().offset()))
        .filter(|indent| indent.len() > parent_indent.len())
        .unwrap_or_else(|| format!("{parent_indent}    "));
    let mut node_offsets = Vec::new();
    collect_node_offsets(moved_node, 0, &mut node_offsets);
    let mut replacements = Vec::new();
    for (name_offset, depth) in node_offsets {
        if name_offset < moved_start {
            continue;
        }
        let line_start = original_source[..name_offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        if !original_source[line_start..name_offset]
            .chars()
            .all(|ch| matches!(ch, ' ' | '\t'))
        {
            continue;
        }
        let local_line_start = line_start
            .checked_sub(moved_start)
            .ok_or_else(|| DocumentError::UnsafeValue("cannot reindent a moved node".into()))?;
        let local_name_offset = name_offset - moved_start;
        if local_name_offset > moving.len() || local_line_start > local_name_offset {
            return Err(DocumentError::UnsafeValue(
                "cannot reindent a moved node".into(),
            ));
        }
        let target_indent = format!("{child_indent}{}", "    ".repeat(depth));
        replacements.push((local_line_start..local_name_offset, target_indent));
    }
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut adjusted = moving.to_owned();
    for (range, indent) in replacements {
        adjusted.replace_range(range, &indent);
    }
    Ok(adjusted)
}

fn collect_node_offsets(node: &KdlNode, depth: usize, offsets: &mut Vec<(usize, usize)>) {
    offsets.push((node.name().span().offset(), depth));
    if let Some(children) = node.children() {
        for child in children.nodes() {
            collect_node_offsets(child, depth + 1, offsets);
        }
    }
}

fn line_ending_near(source: &str, offset: usize) -> &'static str {
    let before = &source[..offset.min(source.len())];
    if let Some(line_break) = before.rfind('\n') {
        return if before.as_bytes().get(line_break.saturating_sub(1)) == Some(&b'\r') {
            "\r\n"
        } else {
            "\n"
        };
    }
    if source[offset.min(source.len())..].find("\r\n").is_some() {
        "\r\n"
    } else {
        "\n"
    }
}

fn normalize_line_endings_outside_strings(source: &str, line_ending: &str) -> String {
    #[derive(Clone, Copy)]
    enum StringState {
        None,
        Quoted,
        Multiline,
        Raw { hashes: usize, multiline: bool },
    }

    let bytes = source.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    let mut string_state = StringState::None;
    let mut block_comment_depth = 0usize;
    let mut line_comment = false;
    while index < bytes.len() {
        if block_comment_depth > 0 {
            if bytes.get(index..index + 2) == Some(b"/*") {
                block_comment_depth += 1;
                output.extend_from_slice(b"/*");
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                block_comment_depth -= 1;
                output.extend_from_slice(b"*/");
                index += 2;
            } else if bytes[index] == b'\n' || bytes[index] == b'\r' {
                index = push_normalized_line_ending(bytes, index, line_ending, &mut output);
            } else {
                output.push(bytes[index]);
                index += 1;
            }
            continue;
        }
        if line_comment {
            if bytes[index] == b'\n' || bytes[index] == b'\r' {
                index = push_normalized_line_ending(bytes, index, line_ending, &mut output);
                line_comment = false;
            } else {
                output.push(bytes[index]);
                index += 1;
            }
            continue;
        }

        match string_state {
            StringState::Quoted => {
                if bytes[index] == b'\\' {
                    output.push(bytes[index]);
                    index += 1;
                    if index < bytes.len() {
                        if bytes[index] == b'\n' || bytes[index] == b'\r' {
                            index =
                                push_normalized_line_ending(bytes, index, line_ending, &mut output);
                        } else {
                            output.push(bytes[index]);
                            index += 1;
                        }
                    }
                } else if bytes[index] == b'"' {
                    output.push(bytes[index]);
                    index += 1;
                    string_state = StringState::None;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            StringState::Multiline => {
                if bytes.get(index..index + 3) == Some(b"\"\"\"") {
                    output.extend_from_slice(b"\"\"\"");
                    index += 3;
                    string_state = StringState::None;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            StringState::Raw { hashes, multiline } => {
                let quote_width = if multiline { 3 } else { 1 };
                let quote_matches = if multiline {
                    bytes.get(index..index + 3) == Some(b"\"\"\"")
                } else {
                    bytes[index] == b'"'
                };
                let hashes_start = index + quote_width;
                let hashes_match = hashes_start + hashes <= bytes.len()
                    && bytes[hashes_start..hashes_start + hashes]
                        .iter()
                        .all(|byte| *byte == b'#');
                if quote_matches && hashes_match {
                    output.extend_from_slice(&bytes[index..hashes_start + hashes]);
                    index = hashes_start + hashes;
                    string_state = StringState::None;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            StringState::None => {
                if bytes.get(index..index + 2) == Some(b"//") {
                    output.extend_from_slice(b"//");
                    index += 2;
                    line_comment = true;
                } else if bytes.get(index..index + 2) == Some(b"/*") {
                    output.extend_from_slice(b"/*");
                    index += 2;
                    block_comment_depth = 1;
                } else if bytes[index] == b'#'
                    && let Some((hashes, multiline, open_width)) = raw_string_open(bytes, index)
                {
                    output.extend_from_slice(&bytes[index..index + open_width]);
                    index += open_width;
                    string_state = StringState::Raw { hashes, multiline };
                } else if bytes.get(index..index + 3) == Some(b"\"\"\"") {
                    output.extend_from_slice(b"\"\"\"");
                    index += 3;
                    string_state = StringState::Multiline;
                } else if bytes[index] == b'"' {
                    output.push(bytes[index]);
                    index += 1;
                    string_state = StringState::Quoted;
                } else if bytes[index] == b'\n' || bytes[index] == b'\r' {
                    index = push_normalized_line_ending(bytes, index, line_ending, &mut output);
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
        }
    }
    String::from_utf8(output).expect("line ending normalization preserves UTF-8")
}

fn raw_string_open(bytes: &[u8], start: usize) -> Option<(usize, bool, usize)> {
    let mut quote = start;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    let hashes = quote - start;
    if hashes == 0 || bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let multiline = bytes.get(quote..quote + 3) == Some(b"\"\"\"");
    let quote_width = if multiline { 3 } else { 1 };
    Some((hashes, multiline, quote + quote_width - start))
}

fn push_normalized_line_ending(
    bytes: &[u8],
    start: usize,
    line_ending: &str,
    output: &mut Vec<u8>,
) -> usize {
    if bytes.get(start..start + 2) == Some(b"\r\n") {
        output.extend_from_slice(line_ending.as_bytes());
        start + 2
    } else {
        output.extend_from_slice(line_ending.as_bytes());
        start + 1
    }
}

fn line_indent(source: &str, offset: usize) -> String {
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    source[line_start..offset]
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .collect()
}

fn node_line_range(source: &str, node: &KdlNode) -> Result<(usize, usize), DocumentError> {
    let name_start = node.name().span().offset();
    let line_start = source[..name_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let starts_on_own_line = source[line_start..name_start]
        .chars()
        .all(|ch| matches!(ch, ' ' | '\t'));
    let start = if starts_on_own_line {
        line_start
    } else {
        node.span().offset()
    };
    let span_end = node.span().offset().saturating_add(node.span().len());
    if span_end > source.len() || !source.is_char_boundary(span_end) {
        return Err(DocumentError::UnsafeValue(
            "KDL parser returned an invalid node span".into(),
        ));
    }
    let line_end = source[span_end..]
        .find('\n')
        .map_or(source.len(), |index| span_end + index);
    let bytes = source.as_bytes();
    let mut cursor = span_end;
    let mut owned_end = span_end;
    while cursor < line_end {
        match bytes[cursor] {
            b' ' | b'\t' | b'\r' => cursor += 1,
            b';' if owned_end == span_end => {
                cursor += 1;
                owned_end = cursor;
            }
            b'/' if bytes.get(cursor + 1) == Some(&b'/') => {
                cursor = line_end;
            }
            b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                cursor = skip_block_comment(bytes, cursor, line_end)?;
                owned_end = cursor;
            }
            _ => {
                return Ok((start, owned_end));
            }
        }
    }
    let end = if line_end < source.len() {
        line_end + 1
    } else {
        line_end
    };
    Ok((start, end))
}

fn node_source_start(source: &str, node: &KdlNode) -> usize {
    let name_start = node.name().span().offset();
    let line_start = source[..name_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    if source[line_start..name_start]
        .chars()
        .all(|ch| matches!(ch, ' ' | '\t'))
    {
        line_start
    } else {
        name_start
    }
}

fn node_header_end(source: &str, node: &KdlNode) -> Result<usize, DocumentError> {
    let start = node.name().span().offset();
    let end = start.saturating_add(node.span().len()).min(source.len());
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < end {
        match bytes[cursor] {
            b'{' => return Ok(cursor),
            b'\n' | b';' => return Ok(cursor),
            b'/' if bytes.get(cursor + 1) == Some(&b'/') => return Ok(cursor),
            b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                cursor = skip_block_comment(bytes, cursor, end)?
            }
            b'"' | b'#' => cursor = skip_quoted(bytes, cursor, end),
            _ => cursor += 1,
        }
    }
    Ok(end)
}

fn node_children_close(source: &str, node: &KdlNode) -> Result<usize, DocumentError> {
    let start = node.span().offset();
    let end = start.saturating_add(node.span().len()).min(source.len());
    let bytes = source.as_bytes();
    let mut cursor = start;
    let mut open = None;
    let mut depth = 0usize;
    while cursor < end {
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'/') {
            cursor += 2;
            while cursor < end && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            continue;
        }
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            cursor = skip_block_comment(bytes, cursor, end)?;
            continue;
        }
        if matches!(bytes[cursor], b'"' | b'#') {
            cursor = skip_quoted(bytes, cursor, end);
            continue;
        }
        match bytes[cursor] {
            b'{' => {
                open.get_or_insert(cursor);
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    return Ok(cursor);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    Err(DocumentError::UnsafeValue(
        "could not locate child block close".into(),
    ))
}

fn skip_quoted(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut cursor = start;
    if bytes[cursor] == b'#' {
        let raw_start = cursor;
        while cursor < end && bytes[cursor] == b'#' {
            cursor += 1;
        }
        if cursor >= end || bytes[cursor] != b'"' {
            return raw_start + 1;
        }
        let hashes = cursor - raw_start;
        cursor += 1;
        while cursor < end {
            if bytes[cursor] == b'"'
                && bytes
                    .get(cursor + 1..cursor + 1 + hashes)
                    .is_some_and(|tail| tail.iter().all(|byte| *byte == b'#'))
            {
                return cursor + 1 + hashes;
            }
            cursor += 1;
        }
        return end;
    }
    let multiline = bytes.get(cursor..cursor + 3) == Some(b"\"\"\"");
    cursor += if multiline { 3 } else { 1 };
    while cursor < end {
        if multiline && bytes.get(cursor..cursor + 3) == Some(b"\"\"\"") {
            return cursor + 3;
        }
        if !multiline && bytes[cursor] == b'"' {
            return cursor + 1;
        }
        if !multiline && bytes[cursor] == b'\\' {
            cursor += 1;
        }
        cursor += 1;
    }
    end
}

fn entry_string_range(source: &str, entry: &KdlEntry) -> Result<(usize, usize), DocumentError> {
    let span = entry.span();
    let start = span.offset();
    let end = start.saturating_add(span.len());
    if end > source.len() || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
        return Err(DocumentError::UnsafeValue(
            "KDL parser returned an invalid source span".into(),
        ));
    }
    let range = if let Some(name) = entry.name() {
        let name_end = name.span().offset().saturating_add(name.span().len());
        find_attribute_value(source, name_end, end)?
    } else {
        find_string_token(source, start, end)?
    };
    Ok(range)
}

fn encode_kdl_string(value: &str) -> String {
    let canonical = KdlValue::String(value.to_owned()).to_string();
    if canonical.starts_with('"') || canonical.starts_with('#') {
        canonical
    } else {
        format!("\"{value}\"")
    }
}

fn entry_value_range(source: &str, entry: &KdlEntry) -> Result<(usize, usize), DocumentError> {
    if entry.name().is_some() {
        let span = entry.span();
        let start = entry.name().unwrap().span().offset() + entry.name().unwrap().span().len();
        let end = span.offset().saturating_add(span.len()).min(source.len());
        return find_attribute_value_token(source, start, end);
    }
    let span = entry.span();
    Ok((span.offset(), span.offset().saturating_add(span.len())))
}

fn find_attribute_value(
    source: &str,
    start: usize,
    end: usize,
) -> Result<(usize, usize), DocumentError> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    loop {
        while cursor < end && matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n') {
            cursor += 1;
        }
        if cursor + 1 < end && bytes[cursor..cursor + 2] == *b"/*" {
            cursor = skip_block_comment(bytes, cursor, end)?;
        } else if bytes.get(cursor..cursor + 2) == Some(b"//") {
            while cursor < end && bytes[cursor] != b'\n' {
                cursor += 1;
            }
        } else if bytes.get(cursor) == Some(&b'=') {
            return find_string_token(source, cursor + 1, end);
        } else if cursor >= end {
            break;
        } else {
            cursor += 1;
        }
    }
    Err(DocumentError::UnsafeValue(
        "could not locate attribute value".into(),
    ))
}

fn find_attribute_value_token(
    source: &str,
    start: usize,
    end: usize,
) -> Result<(usize, usize), DocumentError> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    loop {
        while cursor < end && matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n') {
            cursor += 1;
        }
        if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            cursor = skip_block_comment(bytes, cursor, end)?;
        } else if bytes.get(cursor..cursor + 2) == Some(b"//") {
            while cursor < end && bytes[cursor] != b'\n' {
                cursor += 1;
            }
        } else if bytes.get(cursor) == Some(&b'=') {
            cursor += 1;
            break;
        } else if cursor < end {
            cursor += 1;
        } else {
            return Err(DocumentError::UnsafeValue(
                "could not locate attribute value".into(),
            ));
        }
    }

    loop {
        while cursor < end && matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n') {
            cursor += 1;
        }
        if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            cursor = skip_block_comment(bytes, cursor, end)?;
        } else if bytes.get(cursor..cursor + 2) == Some(b"//") {
            while cursor < end && bytes[cursor] != b'\n' {
                cursor += 1;
            }
        } else {
            break;
        }
    }

    let value_start = cursor;
    if cursor >= end {
        return Err(DocumentError::UnsafeValue(
            "attribute has no value token".into(),
        ));
    }
    if bytes[cursor] == b'#' || bytes[cursor] == b'"' {
        if bytes[cursor] == b'#' {
            let mut quote = cursor;
            while quote < end && bytes[quote] == b'#' {
                quote += 1;
            }
            if bytes.get(quote) == Some(&b'"') {
                return Ok((value_start, skip_quoted(bytes, value_start, end)));
            }
        } else {
            return Ok((value_start, skip_quoted(bytes, value_start, end)));
        }
    }
    while cursor < end && !matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n' | b';' | b'}') {
        if bytes.get(cursor..cursor + 2) == Some(b"//")
            || bytes.get(cursor..cursor + 2) == Some(b"/*")
        {
            break;
        }
        cursor += 1;
    }
    (cursor > value_start)
        .then_some((value_start, cursor))
        .ok_or_else(|| DocumentError::UnsafeValue("attribute has no value token".into()))
}

fn find_string_token(
    source: &str,
    start: usize,
    end: usize,
) -> Result<(usize, usize), DocumentError> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < end {
        match bytes[cursor] {
            b' ' | b'\t' | b'\r' | b'\n' => cursor += 1,
            b'/' if cursor + 1 < end && bytes[cursor + 1] == b'/' => {
                cursor += 2;
                while cursor < end && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            b'/' if cursor + 1 < end && bytes[cursor + 1] == b'*' => {
                cursor = skip_block_comment(bytes, cursor, end)?;
            }
            b'(' => {
                cursor += 1;
                while cursor < end && bytes[cursor] != b')' {
                    cursor += 1;
                }
                cursor = (cursor + 1).min(end);
            }
            b'#' => {
                let token_start = cursor;
                while cursor < end && bytes[cursor] == b'#' {
                    cursor += 1;
                }
                if cursor < end && bytes[cursor] == b'"' {
                    let hashes = cursor - token_start;
                    cursor += 1;
                    while cursor < end {
                        if bytes[cursor] == b'"'
                            && bytes
                                .get(cursor + 1..cursor + 1 + hashes)
                                .is_some_and(|tail| tail.iter().all(|byte| *byte == b'#'))
                        {
                            return Ok((token_start, cursor + 1 + hashes));
                        }
                        cursor += 1;
                    }
                }
            }
            b'"' => {
                let token_start = cursor;
                let multiline = bytes.get(cursor..cursor + 3) == Some(b"\"\"\"");
                cursor += if multiline { 3 } else { 1 };
                while cursor < end {
                    if multiline && bytes.get(cursor..cursor + 3) == Some(b"\"\"\"") {
                        return Ok((token_start, cursor + 3));
                    }
                    if !multiline && bytes[cursor] == b'"' {
                        return Ok((token_start, cursor + 1));
                    }
                    if bytes[cursor] == b'\\' {
                        cursor += 1;
                    }
                    cursor += 1;
                }
            }
            _ => cursor += 1,
        }
    }
    Err(DocumentError::UnsafeValue(
        "could not locate string value in source".into(),
    ))
}

fn skip_block_comment(bytes: &[u8], mut cursor: usize, end: usize) -> Result<usize, DocumentError> {
    let mut depth = 0usize;
    while cursor + 1 < end {
        if bytes[cursor..cursor + 2] == *b"/*" {
            depth += 1;
            cursor += 2;
        } else if bytes[cursor..cursor + 2] == *b"*/" {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                return Ok(cursor);
            }
        } else {
            cursor += 1;
        }
    }
    Err(DocumentError::UnsafeValue(
        "unterminated comment in attribute".into(),
    ))
}

fn diagnostic(
    source: &str,
    kind: DiagnosticKind,
    message: String,
    offset: usize,
    length: usize,
    file_name: Option<String>,
    slide_index: Option<usize>,
    slide_id: Option<String>,
    element_index: Option<usize>,
    column_index: Option<usize>,
    nested_element_index: Option<usize>,
) -> Diagnostic {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let column = source[line_start..offset].chars().count() + 1;
    let source_line = source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or_default()
        .to_owned();
    Diagnostic {
        kind,
        message,
        file_name,
        line,
        column,
        source_line,
        slide_index,
        slide_id,
        element_index,
        column_index,
        nested_element_index,
        byte_range: offset..offset.saturating_add(length).min(source.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_rechecks_changes_after_the_temporary_file_is_synced() {
        for delete in [false, true] {
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("presentation.kdl");
            let original = "presentation { metadata { title \"Original\" }; slide {} }";
            let external = original.replace("Original", "External");
            fs::write(&path, original).unwrap();
            let mut document = PresentationDocument::open(&path).unwrap();
            document.set_title("Draft").unwrap();
            let draft = document.source().to_owned();
            let error = document
                .save_before_replace(|| {
                    // Prove the injected change happens after writing the complete draft.
                    let temp = fs::read_dir(folder.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .find(|candidate| candidate != &path)
                        .unwrap();
                    assert_eq!(fs::read(temp).unwrap(), draft.as_bytes());
                    if delete {
                        fs::remove_file(&path).unwrap();
                    } else {
                        fs::write(&path, &external).unwrap();
                    }
                })
                .unwrap_err();
            if delete {
                assert!(
                    matches!(error, DocumentError::Io(ref error) if error.kind() == std::io::ErrorKind::NotFound)
                );
                assert!(!path.exists());
            } else {
                assert!(matches!(error, DocumentError::ExternalChange));
                assert_eq!(fs::read(&path).unwrap(), external.as_bytes());
            }
            assert_eq!(document.source(), draft);
            assert!(document.is_dirty());
            assert!(document.can_undo());
            assert_eq!(document.path(), Some(path.as_path()));
            assert_eq!(
                fs::read_dir(folder.path()).unwrap().count(),
                usize::from(!delete)
            );
            // Restoring the baseline permits a retry, without losing the edit's Undo.
            fs::write(&path, original).unwrap();
            document.save().unwrap();
            assert_eq!(fs::read(&path).unwrap(), draft.as_bytes());
            assert!(!document.is_dirty());
            assert!(document.undo());
            assert_eq!(document.source(), original);
            assert!(document.redo());
            assert_eq!(document.source(), draft);
        }
    }

    #[test]
    fn string_token_finder_handles_raw_and_attribute_strings() {
        let source = "title key= /* keep */ #\"日本\"##";
        let range = find_attribute_value(source, 6, source.len()).unwrap();
        assert_eq!(&source[range.0..range.1], "#\"日本\"#");
    }

    #[test]
    fn numeric_attribute_token_finder_leaves_comments_outside_the_edit_range() {
        let source = "column width /* before equals */ = /* before value */ 50";
        let range = find_attribute_value_token(source, 7, source.len()).unwrap();

        assert_eq!(&source[range.0..range.1], "50");
        assert!(source[..range.0].contains("/* before value */"));
    }
}
