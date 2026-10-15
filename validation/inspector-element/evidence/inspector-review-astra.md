# 独立レビュー

対象: `/tmp/psycho-issue-53-review`、`c64a70f..ed055bd`。#53、親#48、#67、指定rubric/code-quality/lead-judgmentを確認。ソース変更・GUI操作なし。固定GPUI rev `14dd03e` の実装も追跡した。

## 1. [critical] 複数のColumnsで構造一覧のアクセシビリティIDが衝突する

**場所**: `src/main.rs:3014` (`render_outline_row`)、`src/main.rs:5766` (`outline_rows`)、`src/main.rs:6287` (`styled_button`)

**具体的な発生条件**: 1枚のSlideにColumnsを2つ置き、それぞれの左右列に本文を1つずつ入れる。構造一覧は「Element 1 · 2列」「左列 · 本文 1」「右列 · 本文 1」「Element 2 · 2列」「左列 · 本文 1」「右列 · 本文 1」となる。未選択状態でアクセシビリティを有効にして一覧を表示する。

**根拠**: 列内ラベルは親Columnsの識別情報を含まない。`render_outline_row`はラベルを`styled_button`へ渡し、共通buttonは`.id(label.clone())`を設定する。全行が同じ`element-outline`直下のため同一GlobalElementIdになる。固定GPUI `elements/div.rs:2325`はGlobalElementIdからaccesskit NodeIdを生成する。`window/a11y.rs:411-422`の`can_push`は重複IDでdebug_assertに失敗し、releaseでは後発ノードを破棄する。従って開発ビルドではVoiceOver/AX利用時のクラッシュ、releaseでは後ろの列内Elementの欠落が実装から確定する。表示時にチェックマークが付く選択行だけ別IDになることは、未選択行同士の衝突を解消しない。

**分類**: ラベル依存IDという素因は既存だが、旧4件ページでは上記fixtureの同名行は別ページだった。全件同時マウントによって衝突条件が広がった本変更の回帰。

**最小修正**: `render_outline_row`で`CanvasSource`のslide/parent/column/indexから作った安定した一意IDを`.id(...)`で指定する。表示ラベルと状態識別子を分ける。2組のColumnsを置いてアクセシビリティツリーを読み、全行の選択先を照合する。

## 2. [critical] 設定内のTab移動で画面外の操作が表示されない

**場所**: `src/main.rs:3085` (`inspector-settings`)、`src/main.rs:2928` (FocusNext)

**具体的な発生条件**: 30行のコード全文、または多数の箇条書き項目があるElementを構造一覧で選択し、Inspectorの設定をTabで下へ移動する。適用・言語・削除など入力部品より下方の操作へ進む。

**根拠**: 設定領域は`overflow_y_scroll`と`inspector_scroll`を持つが、フォーカスに応じたスクロール処理がない。`inspector_scroll`への書き込みは選択Element変更時のoffset=0のみ。一方、構造一覧は明示的に`on_focus`→`scroll_to_item(index)`を実装している。アプリのTabは`Window::focus_next`を呼ぶだけで、固定GPUI `window.rs:2336`→`focus`はfocus ID更新・refreshのみ、自動スクロールを行わない。従って画面外の設定へフォーカスは進んでも、フォーカス枠と操作内容が見えない。#53のTabで全文編集・設定へ到達する受入要件を満たさない。実機による同経路の確認は未実施であり、最終判断前にownerのkeyboard acceptanceで画面照合が必要。

**分類**: 旧Inspectorにも共通の自動スクロールはなく、完全な新規回帰とは断定しない。本変更は構造一覧側のみ対策しており、選択対象設定の新しいスクロール領域に既存欠陥を持ち越している。

**最小修正**: 設定内のフォーカス対象をScrollHandleへ接続し、フォーカスした部品を可視領域に出す。大量のコード／箇条書きを使いTabのみで下方操作の名前・フォーカス・実行を確認する。

## 既知・修正待ち

Ownerから報告された無効列幅のままInspectorを閉じられる問題はコードでも追跡できる。`select_columns`/`select_column_width`が`canvas_editing`をクリアせず、`select_current_field`も常にedit=trueを渡す。`toggle_pane`の`!canvas_editing`条件をすり抜ける。これは既存選択ヘルパー由来だが#53の契約に関係し、ownerが既に修正中との連絡を受けた。今回追加発見として重複計上しない。

## 証拠の限界と構造判断

`issue-53-complete-5/observations.json`で長い構造の末尾、左右列、未対応言語とプリセット、無効列幅の証跡を読んだ。標準日本語IME往復・VoiceOver・Tabの最終合格証跡はこの時点では未確認。GUI操作はownerと競合させず実施していない。

main.rsは変更前から1000行を超えており、今回「1000行未満から超えた」問題ではない。種類別設定をmatchへ集め、source/elementの選択対応を共通化しているため、ファイルサイズだけを追加blockerにはしない。大規模分割要求は今回の正しさを改善する具体的根拠に乏しく、提起しない。
