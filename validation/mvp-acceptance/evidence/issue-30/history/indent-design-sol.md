# Indentation of moved KDL source: design sketch

## Caller's view

The existing public `add_element` and `move_*` calls keep their signatures. A move still cuts the exact `node_line_range`, reparses the remaining candidate, adjusts only owned layout bytes in the cut slice, then calls `insert_moved_source_at` and the existing commit path. `child_insertion` uses the same destination indentation policy for newly created children. No whole-document formatter runs.

## Grounded failure

- `reindent_moved_source` currently finds node **names** with `collect_node_offsets` and replaces indentation only where a name starts a line. It misses owned comment-only lines and closing braces (`02_01`). It also misses the first line of a node after a semicolon, because that name is not preceded solely by whitespace (`02_03`). The same first-line defect appears before a multiline quoted value (`10_01`).
- `child_insertion` and `reindent_moved_source` independently use four spaces when a destination has no child. A tab-indented document therefore gets a space-indented first child (`07_02`, `10_01`).
- `node_line_range` deliberately starts at the name of an inline node and includes its same-line trailing comment, while a separate comment above an own-line node stays outside the cut. Preserve those ownership decisions. `insert_moved_source_at` chooses the insertion position and line ending; it should not guess an indentation policy.
- `normalize_line_endings_outside_strings` already has a lexer-like state machine. Its escaped-newline branch currently calls `push_normalized_line_ending` while `StringState::Quoted`; the proposed shared scanner must treat even escaped newlines inside a quoted token as protected bytes. The move must preserve **every** quoted token interior byte, including raw/multiline and disabled (`/-`) values.

## Private sketch

```rust
struct IndentStyle { parent: String, child: String }

// A pure description of changes to the extracted source slice. No mutation.
struct TriviaIndentEdit { range: Range<usize>, replacement: String }

fn destination_indent(source: &str, doc: &KdlDocument, parent: &KdlNode) -> IndentStyle;
fn plan_moved_trivia_indent(
    original: &str,
    node: &KdlNode,
    cut: Range<usize>,
    destination_child_indent: &str,
) -> Result<Vec<TriviaIndentEdit>, DocumentError>;
fn apply_trivia_edits(moving: &str, edits: &[TriviaIndentEdit]) -> String;
```

`destination_indent` first uses an existing direct child's own-line prefix. For an empty parent, infer the indent step from the nearest structurally comparable parent/child pair in the parsed document: prefer a sibling parent at the same depth (for example, the populated slide beside an empty slide), then a nearby ancestor's child edge. A valid edge has two own-line prefixes where the child prefix starts with the parent prefix; the suffix is the observed step (`"\t"` or spaces). Fall back to four spaces only when the document has no usable edge. Keep this policy in one helper, used by both insertion and moving. The document argument can be threaded through private call sites; no public API changes.

The planner scans the *original* source with one KDL-aware lexical classification shared with line-ending conversion: ordinary quoted strings, multiline strings, raw hash strings, escaped characters, `//`, and nested `/* */`, including disabled tokens. It produces edits only for leading horizontal whitespace on physical lines inside `cut` whose first non-whitespace byte is outside a quoted token. A comment-only line is eligible when it lies inside the node's owned span; a separate preceding comment is outside `cut`. Closing-brace lines are eligible. A newline and leading spaces inside any quoted token are never eligible. Same-line trailing comments stay byte-identical except for the first line's prefix.

Let `old_base` be the whitespace from the moved node's physical line start to its first non-whitespace byte, even when another node precedes it on the line. For each eligible continuation line whose prefix starts with `old_base`, replace only that prefix with `destination_child_indent + prefix[old_base.len()..]`. This retains the source's relative whitespace instead of reconstructing indentation from tree depth. For a first line cut at an inline node name, insert `destination_child_indent` at local offset zero; for an own-line cut, replace its `old_base` prefix. Prefixes that do not share `old_base` are left unchanged and can be tested as irregular source layout, rather than silently rewritten. Edits use original slice byte offsets, are checked for overlap and UTF-8 boundaries, and apply in descending order.

`reindent_moved_source` becomes a small consumer of `destination_indent` and the pure edit plan. `insert_moved_source_at` remains responsible for insertion and terminator selection. Its line-ending normalization should consume the shared lexical classifications, changing only line endings outside quoted tokens; it must not normalize an escaped newline while inside `Quoted`. The order is: plan edits in original coordinates, apply to the cut slice, then normalize unprotected line endings to the destination's convention, then insert. No edit is planned against the reparsed candidate's offsets.

## Expected evidence

`02_01` moves the image's internal comment, caption, and close brace from 8/12 to 16/20 spaces while the independent comment remains in place. `02_03` prefixes the inline `text` with 16 spaces and retains its trailing comment. `07_02` infers a tab from the populated sibling slide for the empty slide. `10_01` prefixes `code` with four tabs and keeps all physical lines inside its quoted tokens byte-identical. The four strengthened regression tests should pass by exact source comparison; their undo/redo and saved bytes assertions also check ownership and history.

## Design check

The public interface stays small, and callers do not coordinate several low-level stages. One helper owns indentation inference, one pure planner owns external trivia edits, and one scanner owns quoted-token boundaries; this avoids the present duplicated four-space policy and divergent token handling. The plan is deliberately scoped to an extracted node slice, so it cannot format unrelated source or adopt an independent comment.

## Accepted refinement during implementation

独立レビューは行コメントの終了判定がCR/LFだけであるため、U+2028の後にあるraw文字列の空白を再インデントする再現を確認した。KDLの8種の改行を引用走査に反映する。さらに移動範囲・挿入位置・行頭推定にも同じ改行の規則を適用し、公開操作の失敗を先に記録する。根拠は親仕様#17のUnicode改行と任意の有効な挿入位置、`tests/moved_trivia.rs`と`target/issue30/newline-boundaries-before.log`。切り出し範囲と未編集バイトを保つ責任、公開API不変の契約は維持する。
