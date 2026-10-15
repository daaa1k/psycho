# Issue 53 設計候補

## 利用例（仕様）

```rust
// 未選択。タイトルと現在 Slide の構造を示す。背景クリックで選択を外す。
app.inspector_context() == InspectorContext::Presentation { slide: 2 }
app.select_background(window, cx); // 既存 draft を commit できた場合だけ選択解除

// 構造一覧から画面外の右列コードを選ぶ。選択・スクロール・Inspector が同じ対象を指す。
app.select_outline(OutlineTarget::Nested { slide: 2, columns: 1, side: Right, index: 8 }, window, cx);
// canvas 側の同じ対象を選んだ場合も同じ選択関数を通る
app.select_canvas_source(source, window, cx);

// 幅変更・Inspector の開閉は編集中の選択と draft を保持する。
app.resize_pane(Pane::Inspector, width, window, cx);
app.toggle_pane(Pane::Inspector, window, cx); // 既存 commit_draft の成否を尊重。IME marked text 中は先に IME に処理させる
```

選択後は kind に対応する設定だけを描く。文字は既存の共通 TextInput、画像は参照と caption、コードは保存済み言語文字列を保つ picker と任意文字列入力、箇条書きは項目操作、Columns は数値幅と既存ドラッグを示す。無効値は理由と取消操作を表示する。選択遷移は既存 `commit_draft` を唯一の確定境界とし、失敗なら旧選択と入力を保持する。

## 型と署名

既存 `EditTarget` を唯一の編集中対象とし、Title は presentation 設定として維持する。別の document/editor/history state を設けない。新規には選択解除を加える案:

```rust
enum InspectorContext {
    Presentation { slide: usize },
    Element(EditTarget),
}

// context は self.target と current_slide から導出。保存しない・同期しない。
fn inspector_context(&self) -> InspectorContext; // not implemented
fn select_background(&mut self, window: &mut Window, cx: &mut Context<Self>); // commit_draft 成功時に Title へ
fn select_outline(&mut self, target: OutlineTarget, window: &mut Window, cx: &mut Context<Self>); // not implemented

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ColumnSide { Left, Right }
enum OutlineTarget {
    Element { slide: usize, index: usize },
    Nested { slide: usize, columns: usize, side: ColumnSide, index: usize },
}

fn render_element_outline(&mut self, model: &PresentationModel, cx: &mut Context<Self>) -> AnyElement;
fn render_inspector(&mut self, model: Option<&PresentationModel>, cx: &mut Context<Self>) -> AnyElement;
```

`InspectorContext` adapter/enum is optional: since `EditTarget::Title` already means no Element selected, prefer a small accessor or direct match unless it reduces branching materially. OutlineTarget can replace `CanvasSource` only if it has identical identity semantics; otherwise conversion at selection boundary. `CanvasSource` and `EditTarget` remain distinct because canvas hit target lacks field/width edit mode.

## 責務と配置

- `main.rs`: selection transitions, shared `self.target`, `commit_draft`, inspector settings; no new document state.
- `render_element_outline`: remove PAGE_SIZE/OutlinePage; render all rows in a vertically scrolling viewport with existing scroll infrastructure. A row click uses existing select methods; render selected semantics/role (not a checkmark as the sole state), and scroll selected row into view on canvas-driven selection. Preserve nested row indentation and label.
- Canvas background hit test: route only empty canvas surface to `select_background`; element hit targets continue through `select_canvas_source`. Avoid overlay stealing element clicks.
- `input.rs`: add contextual accessible label/placeholder setter to shared TextInput only if current `TextInput` API cannot express image path, language, width, or bullet labels. Keep one editor entity and existing marked-text/composition geometry.
- `editor_view.rs`: pane width/visibility remains view state; resizing/toggling must not alter target or editor value. Existing toggle commits before hide; retain this gate.

`OutlineTarget` uses slide/column/element indices because current selection APIs and immutable model addressing already use them. Reconcile selected target against current model after structural edits; never retain stale row identity through an edit that moves/deletes that row. No generic tree framework or new module is warranted for a single outline.

## 判断と代替案

採用候補は既存 Inspector の中で selection を single source of truth にし、構造リストをスクロール可能な連続リストへ置換する形。変更範囲が `main.rs` と必要な時だけ `input.rs` に収まり、既存編集・履歴・IME・外部競合の境界を通る。一覧は current Slide の全 top-level と左右列 descendants を列挙し、選択行へ `scroll_to_item(Nearest)` 相当を適用する。画面外項目にも到達可能で、描画範囲は既存 scroll の可視化に任せる。

構造的に異なる案は、`InspectorState { selection: Option<SelectionPath>, draft: ... }` を新設し、Element tree の domain path を永続 selection として持ち、Inspector を別 view/component に切る形。長期的に複数選択や rename を足す場合は自然だが、現状では `EditTarget` と draft の二重状態、選択移行同期、入力 entity の引き渡しを増やす。親 issue は大規模クレート分割を要求せず既存編集エンジン再利用を明記しているため、この issue には層が過剰。

主なリスクは index 参照の構造変更時 stale 化。既存の選択 API に集約し、delete/move/insert 成功後に選択を既存規則で解決すれば局所化できる。IME は selection/pane transition が `commit_draft` に入る前に marked composition を既存優先順位で処理すること、Inspector と Canvas が同一 editor entity を参照することが契約。VoiceOver は選択状態を semantic selected として公開し、TextInput に入力ごとの名前・制約を付ける。色や ✓ だけでは状態・名称を伝えない。

実装時は #53 acceptance の実 GUI 操作、標準日本語 IME の往復、VoiceOver、Undo/保存/再読み込みを既存 acceptance 経路で確認し、KDL と画面配置の証跡を残す。
