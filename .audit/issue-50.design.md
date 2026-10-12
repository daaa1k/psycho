# 共通描画を直接使う Navigator

## Problem

利用者は画像、コード、2列の配置を見て Slide を選ぶ。既存 main.rs の render_canvas_element は Columns と編集処理を同時に扱い、render_element だけでは全要素を描けない。Issue #50 と #48 の描画一致を満たすため、既存の白い Slide 本体を取り出し、キャンバス、発表、一覧が同じ入口を呼ぶ。ソース基点は 343cc05。調査は読み取り専用で行った。

## Usage

以下は提案する呼び出し例であり、現行 API ではない。

```rust
// render_editor 内。preview_model は既存の有効な編集中モデル。
let navigator = self.render_navigator(preview_model.as_ref(), window, cx);
let canvas = self.render_slide_body(
    &preview_model.as_ref().unwrap().slides[self.current_slide],
    self.current_slide,
    self.view.scale(),
    SlideInteraction::Editor,
    &self.asset_images,
    window, cx,
);

// uniform_list の可視範囲コールバック内。
let thumbnail = self.render_slide_body(
    &model.slides[index], index, thumbnail_width / 1280.,
    SlideInteraction::ReadOnly, &self.asset_images, window, cx,
);

// 発表は既存開始時 snapshot の画像を渡す。
let body = self.render_slide_body(
    slide, index, viewport_scale, SlideInteraction::ReadOnly,
    &presentation.render_images, window, cx,
);
```

ReadOnly は画像の取得先を決めない。呼出元がモデルと画像の組を指定するため、一覧が発表中 snapshot を誤用する条件分岐が消える。背景色と中央配置、スクロールなどの周辺 UI は呼出元に残す。

## Shape

`src/navigator.rs` は NavigatorState と PsychoApp の一覧描画・入力処理を持つ非公開モジュールとする。独立 Entity と文書 snapshot を導入せず、既存 PsychoApp が持つモデル、asset_images、current_slide を使う。NavigatorState は FocusHandle、UniformListScrollHandle、最後に公開した選択位置とペイン幅だけを所有する。選択位置の記録はスクロール要求の重複抑制用で、選択の正本は current_slide のままにする。Model the Domain により、フォーカスとスクロールを一つの型に集める。

`src/main.rs` の render_canvas から render_slide_body を抽出する。白背景、1280×720、64×48 の余白、24 の間隔、要素ごとの絶対座標配置をこの入口に集める。既存 render_canvas_element をそのまま通し、ReadOnly で編集枠、入力部品、カーソル、クリック選択を無効にする。Columns の再帰描画も共有する。調査担当の追加指摘により、現在の left/right nested_selected は editing=false を考慮しないため、render_canvas_element の入口と列内選択判定で ReadOnly の selected を必ず false にする。column_resize_drag の preview_width も Editor のときだけ参照する。ReadOnly は model 内の列幅だけを使い、ドラッグ途中の UI 状態をサムネイルや発表へ混ぜない。縮小係数に最小値を足さず、shared_text の基準座標からの glyph 縮小を維持する。欠落画像の文言も共通 shared_text に揃えると、通常 GPUI テキストの縮小時再レイアウトを避けられる。

`render_navigator` は有限高さの親の中に uniform_list を一つ作る。行は同じ幅・同じ高さを持ち、16:9 のサムネイル、外側の番号と一行見出し、固定余白から高さを決める。見出しは全文を保持して UI の省略表示を使い、EditorTooltip に全文を渡す。見出しなしは「スライド N」。空 Slide も同じ白い render_slide_body を使う。基準用の1行の測定は許すが、コールバックが受け取った Range 外のサムネイルは作らない。list に独立した overflow コンテナを重ねない。

uniform_list のコールバックは cx.processor で PsychoApp にアクセスし、その時点の preview_model と asset_images を読む。全100枚の model の clone や100枚の要素生成を closure 作成時に行わない。preview_model の既存 clone コストは描画の仮想化とは別に記録する。paint で cx.notify を呼ばない。

選択、枚数、一覧幅、非表示から再表示への変化だけで scroll_to_item(index, Nearest) を要求する。手動スクロール中に毎フレーム選択行へ戻さない。Undo、Redo、正常な reload、発表終了による current_slide の変化も同じ判定に入る。0枚はスクロールしない。幅変更後には更新された行高で選択を露出する。

行クリック、NavigatorPrevious、NavigatorNext、アクセシビリティの Click は select_navigator_slide の一つの経路に合流する。通常は既存 commit_draft を先に呼び、失敗なら選択とフォーカスを維持して status を表示する。成功後は current_slide を更新し、canvas_editing を解除して EditTarget::Title と既存 editor 値を整合させ、Navigator にフォーカスする。select_title をそのまま呼ぶと root_focus を奪うため、ここで無条件呼び出ししない。外部不一致中は最後の正常モデル内の current_slide だけを変更し、編集対象と draft を保持する。

Navigator の FocusHandle を付けた親に key_context("SlideNavigator") と専用 action handler を付ける。上下と Return の key binding はこのコンテキストだけに限定する。TextInput をこの親の子に置かない。Return は編集開始ではなく既存キャンバス側の root_focus へ進める。もし root_focus が画面全体のままで、実際のキャンバス移動を表せないなら専用 canvas_focus を canvas container に置き、Return の行き先をそこへ変更する。この差分は実装時に記録する。

行の背景と内枠は選択を示す。Navigator にキーボードフォーカスがある場合だけ選択行の外枠を追加する。選択が残っていても Inspector にフォーカスがある場合は外枠を出さない。

GPUI 14dd03e の native AccessKit を使う。親は ListBox、行は ListBoxOption と安定した id、全文の aria_label、aria_selected を持つ。選択行に aria_active_descendant() を付ける。この API は Web と異なり子側に付ける。on_a11y_action の Click を同じ選択入口へ、Focus を選択と Navigator フォーカスへ接続する。子のサムネイル内に不要な accessibility role を付けず、番号・見出し・選択の重複読み上げを避ける。行 id に見出し文字列を含めない。既存 Slide.id がある場合はそれを使い、ない場合は index を使う。index fallback で構造変更時の identity 維持は保証しないが、非同期結果がないため別 Slide への古い内容適用は発生しない。

初回の model None は既存診断 UI を表示して一覧を生成しない。外部無効化の last-good model と不一致バナーを維持する。画像更新は既存 refresh_assets の結果と notify で visible rows に反映する。画面外の行は再露出時に最新モデルと画像を読む。キャッシュや生成タスクがないため generation token は不要。Laziness Protocol により非同期キャッシュと二つ目の画像所有者を追加しない。Boundary Discipline により文書エラー判断は既存 document 境界に残す。

## Synthesis decision

候補 A。統合担当が候補比較後に記入する。

## Tradeoffs accepted

- 可視行を再描画するコストを受け入れ、キャッシュの無効化規則と古い生成結果の競合をなくす。
- PsychoApp を描画の所有者として維持し、既存 callback と編集選択の広範な移植を避ける。Navigator 専用 state とメソッドは別ファイルに集める。
- リスト全体の AX ノードを常駐させず、画面上の行を native AX に公開する。上下キーとスクロールによる末尾到達を実機検証する。

## Alternatives considered

画像生成タスクとサムネイル cache は却下した。モデル、画像、倍率、編集 draft、文書世代のキーと cancellation を公開境界に持ち込み、100枚規模で必要な根拠がまだない。

独立 Navigator Entity がモデル snapshot と選択を保持する案は却下した。イベント往復と二つの選択所有者を生み、commit_draft の失敗時に optimistic selection を巻き戻す責務が増える。

render_element だけを使う簡易表示は却下した。Columns と編集・発表共通の配置を満たさない。

## Implementation reconciliation

実装未着手。canvas の実フォーカス先と、匿名 Slide の identity は上記制約を残す。独立 review と実画面検証前に完了とは扱わない。

## Open questions and risks

VoiceOver が仮想行の選択状態を実際に読み上げ、上下キーで新しく出現した行を追えるか。API の存在は確認済みだが発話は未確認。

可視行の glyph shaping と画像 paint だけで操作に支障が出ないか。100枚は要求された検証規模であり性能測定結果ではない。

## Next implementation step

render_slide_body の抽出だけを先に行い、編集画面と発表の正規化 glyph 配置と画像比率の既存検証を通す。

## Verification

Prove It Works により次の検証を実製品ウィンドウで行う。現時点では計画であり実施結果ではない。

1. 6枚の代表資料と100枚の資料に全要素、両列の各要素、長い見出し、空 Slide、欠落画像を配置する。スクリーンショットと正規化 glyph trace で発表、編集、一覧の改行・座標・画像比率を比較する。
2. 先頭から末尾へ native scroll と上下キーで到達し、選択行の露出と Return 後のキャンバスフォーカスを確認する。描画 trace の Slide index を確認し、可視行と測定行以外を毎フレーム描かないことを確かめる。
3. 文字編集、Undo、Redo、外部正常 reload、同じパスの画像差し替え後に可視行と再露出行の更新を確認する。100枚目を編集してすぐスクロールし、停止や古い表示がない証跡を残す。
4. 初回無効、外部無効、競合中の選択、無効入力による選択拒否、IME変換中の上下と Return を確認する。表示操作前後の KDL と履歴は同一で、入力確定だけ既存履歴に入ることを照合する。
5. VoiceOver を有効にして番号、全文見出し、選択状態の発話を記録する。AX tree だけを発話の代替にしない。
6. 1024×700、1280×860、1440×960、ペイン幅変更、明暗テーマで末尾到達と選択・フォーカスの別枠を確認する。

## Evidence

正本は `gh issue view 50` と `gh issue view 48`。ローカルソースは `/tmp/psycho-issue-50/src/main.rs` の render_slide_list:2923、select_title:573、render_canvas:4140、render_canvas_element:4239 と text.rs の shared_text:54。

GPUI は `/Users/daichikinoshita/.cargo/git/checkouts/zed-a70e2ad075855582/14dd03e/crates/gpui/src/elements/uniform_list.rs` の関数:22、scroll_to_item:150、可視範囲計算:474、track_scroll:683。div.rs の aria_active_descendant:1365、aria_selected:1388、on_a11y_action:1498。gpui_macos/src/window.rs:2365 に native adapter 作成がある。

## 採用判定

gpt-6-astra cross-judge の評価は A24、B22、C14。A を基底とし、B の同フレームの model と images の一致および ReadOnly 不変条件を採用。C の独立 Entity と全scene投影は選択・外部状態の重複と eager layout のため却下。GPUI pinned API は ScrollStrategy::Nearest を持つ。専用 canvas FocusHandle を Return の行き先にする。

## 実装単位

1. shared render_slide_body を明示倍率と SlideInteraction で抽出。既存キャンバス外枠は維持。readonly の selected と drag preview 幅を遮断。
2. NavigatorState の focus/scroll/exposure と可視範囲 uniform_list。current_slide を唯一の選択正本として click/key/AX を共通選択へ集める。
3. native 製品windowの更新・履歴・glyph・100枚・VoiceOverを公開操作で検証。
