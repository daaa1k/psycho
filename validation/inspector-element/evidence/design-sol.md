# Issue 53 設計案

## Problem

現在の `PsychoApp` は `EditTarget` と単一の `TextInputState` を所有する。キャンバスと構造一覧は既に同じ選択経路を使うが、構造一覧は4件ずつのページ送りで、Inspector の大半の操作はボタンの羅列になっている。選択と表示を整理する際も、`commit_draft` が担う IME 確定、入力検証、文書履歴、外部競合の停止を通す必要がある。Issue 48 は表示状態だけで KDL と履歴を変えないことを要求する。

## Usage（呼び出し側）

キャンバスの空白をクリックすると Element 選択が解除され、Inspector に Presentation のタイトルと現在の Slide の構造一覧が出る。一覧の末尾や列内の行までスクロールし、行を押すとキャンバスの選択表示と対象設定が同期する。選択後は種類と編集対象が分かる見出しの下に、適用可能な操作だけが出る。画像は参照と Caption、コードは全文と言語プリセットと任意入力、2列は幅入力と左右の挿入、箇条書きは項目操作を使える。

```rust
// 空白面のクリック。Element のクリック側はイベント伝播を止める。
self.select_title(window, cx); // 既存の選択遷移を通し、未確定入力を先に検証する

// 構造一覧の行。キャンバスと同じ EditTarget に解決される。
self.select_canvas_source(row.source, window, cx);

// コード言語のプリセット。任意入力欄と同じ編集対象、同じ確定経路。
self.select_current_field(ElementField::CodeLanguage, window, cx);
self.editor.update(cx, |input, cx| input.set_value("rust", cx));
self.commit_draft(window, cx)?;
```

プリセットにない `language` は現在値のまま「その他（現在値）」として表示し、選択肢の表示や切替だけでは書き換えない。Inspector を閉じるときに入力中なら確定を試み、失敗時は閉じずに理由を表示する。幅変更と表示切替は選択と入力値を保持する。

## Shape

状態の正本は現行の `PsychoApp.target: EditTarget` と `PsychoApp.editor: Entity<TextInputState>` のまま。`EditTarget::Title` を「Element 未選択」として使う。`CanvasSource` は構造行の位置とキャンバスの位置を表す既存型で、別の選択モデルにしない。`OutlinePage` は削除し、スクロール位置だけを表示状態として保持する。`src/lib.rs` の文書操作や KDL schema は変更しない。

```rust
// src/main.rs。既存 PsychoApp に追加する表示状態。文書には保存しない。
outline_scroll: gpui::ScrollHandle,
outline_exposure: Option<(usize, Option<CanvasSource>)>,

// 1 Slide 分だけ、その時点の PresentationModel から作る。キャッシュしない。
struct OutlineRow {
    source: CanvasSource,
    label: String,
    depth: u8, // 0: Slide 直下、1: 左右列の子
}
fn outline_rows(model: &PresentationModel, slide: usize) -> Vec<OutlineRow>;
fn selected_source(target: EditTarget) -> Option<CanvasSource>;
fn render_element_outline(&mut self, model: &PresentationModel, cx: &mut Context<Self>) -> gpui::AnyElement;

// 選択した場所から設定対象を借用し、種類ごとの表示分岐を一箇所へ寄せる。
fn selected_element<'a>(model: &'a PresentationModel, target: EditTarget) -> Option<&'a Element>;
fn edit_label(target: EditTarget, element: Option<&Element>) -> &'static str;

// src/input.rs。単一入力 Entity に、現在の対象を表す VoiceOver 名を渡す。
impl TextInputState {
    pub fn set_accessible_label(&mut self, label: &'static str, cx: &mut Context<Self>);
}
```

`render_element_outline` は行を全件、専用の高さ制限付き `div().overflow_y_scroll().track_scroll(&self.outline_scroll)` の直接の子として描画する。GPUI の `ScrollHandle::scroll_to_item(index)` は直接の子を指すため、余分なラッパーを挟まない。`outline_exposure` の Slide または選択が変わった時だけ、描画順で `selected_source` に合う行の番号へスクロールする。毎描画時にスクロールすると手動スクロールを奪う。列内行は字下げと「左列／右列」を名前に含め、一覧は `Role::ListBox`、行は `Role::ListBoxOption` と `aria_selected` を使う。既存 Navigator と同じ GPUI の役割付けを踏襲する。長い行の検証で描画負荷が問題になった場合だけ仮想化を検討する。

`render_inspector` は未選択ならタイトル編集と Slide 構造、選択時なら種類と対象を見出しにし、対象に関係する入力と操作を描く。既存の移動・削除・箇条書き操作はそのまま呼ぶ。コード言語プリセットは `&'static [&'static str]` の表示選択肢だけを加え、任意文字列欄は既存の `ElementField::CodeLanguage` と `commit_draft` を共有する。画像パスは現在の相対パス検証、列幅は1〜99の整数検証をそのまま使う。選択または欄の切替時は既存の `select_*` による確定を通す。プリセットの適用もまず前の draft を確定し、失敗時は選択も値も変えない。

`render_slide_body` の背景面に空白クリックを設け、Element の選択・編集・ドラッグのヒット領域では既存の伝播停止を揃える。背景操作は `select_title` を呼ぶだけで、内容変更を直接行わない。`TextInputState` の `.aria_label("編集値")` は対象に応じたラベルへ置き換え、選択遷移時に `edit_label` から設定する。幅は「左列幅、1〜99の整数」のように制約を含める。入力不正の理由と明示取消は既存表示を維持し、操作ボタンの disabled と選択行の状態を VoiceOver に渡す。単一 Entity の表示先切替は、キャンバス編集中と Inspector 編集中に同時マウントしない現行契約を守る。

所有範囲は `src/main.rs` の選択・描画・スクロール表示状態と `src/input.rs` の入力アクセシビリティ名だけ。文書の変更は `PresentationDocument` の既存 API、履歴は `commit_draft` に集中する。呼び出し側は `select_canvas_source` と既存フィールド選択を知ればよく、スクロール行番号、設定の種類分岐、IME 処理を知る必要がない。

## Synthesis decision

この候補は既存の編集状態を正本に保つ形を採る。文書操作を増やさず、行モデルは毎描画時に1 Slide から生成する。別案の独立した Inspector 状態は、選択と draft の同期を増やすため採らない。

## Tradeoffs accepted

- 構造行は描画ごとに再生成する。1 Slide の Element への到達と単一の選択正本を優先する。大量の行で遅延が観測された場合は仮想化する。
- `EditTarget::Title` に未選択の意味を残す。`Option<Selection>` への全面移行を避け、既存の保存・履歴・IME 経路を維持する。
- コード言語プリセットは表示用の固定リストを持つ。保存可能な言語集合を制限せず、任意値を残す。

## Alternatives considered

独立した `InspectorSelection` と `InspectorDraft` を作り、キャンバス側の `EditTarget` へ双方向同期する案。Inspector 単体は整理しやすいが、選択変更と IME 中の入力先移動に二つの書き込み元が生まれる。公開操作も同期・確定・破棄に分かれ、呼び出し側が順序を知る必要がある。現行の単一 `EditTarget` と `commit_draft` の方が境界が小さい。

別案として `uniform_list` で全行を仮想化する形もある。Slide 一覧では有効だが、構造行は字下げや内容で高さが変わり得る。Issue 53 の必須条件は全件への到達であり、先に通常スクロールを実装して実画面で負荷を測る。

## Open questions and risks

- 背景と Element のクリック領域が重なる場所で、背景解除が誤発火しないか。実画面で空白、Caption、列境界、ドラッグを確認する。
- GPUI の選択状態と入力名は macOS VoiceOver で実際に読まれるか。AX ツリーとキーボード操作の両方を確認する。
- 標準日本語 IME の変換中にプリセット選択、ペイン閉鎖、キャンバスと Inspector の移動を行ったとき、確定失敗が draft を保持するか。

## Next implementation step

`OutlinePage` を `ScrollHandle` に置き換え、直接の子で全構造行を描き、行選択とキャンバス選択が同じ `EditTarget` に解決されることから確認する。
