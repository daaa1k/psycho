## Findings

### 1. [warning] 描画のたびに全スライドの配置を再計測する
**Location**: `src/main.rs:5231-5245`, `src/main.rs:6299-6325`
**Finding**: `Render::render` が毎回 `preview_model` を作り、全スライドの全要素に対して `collect_layout_diagnostics` を実行する。文字要素では `measure_text` がテキストの整形を呼ぶ。編集中の一文字ごとの描画でも、変更していない全スライドを再計測する。
**Evidence**: `render_editor` も別途 `preview_model` を呼ぶため、未確定入力のある描画ではソースからの候補文書作成も重複する。親 Issue #48 は100枚の文書で入力・スクロール中の操作性を要求しているが、変更された配置計測は表示中のスライド数に関係なく文書全体へ同期的に広がる。実測の遅延は確認していないため、性能リスクとして扱う。
**Suggestion**: 診断は文書内容・入力候補・計測条件が変わったときに更新し、未変更スライドの結果を保持する。描画関数は計算済みの診断を表示する。

## Audit flags

- `.audit/issue-55.tsv` の先頭行は時刻が過去作業の実施時刻ではなく、後から検証した時刻だと明記している。本文もその解釈と矛盾しない。
- `validation/mvp-acceptance/evidence/issue-55/README.md` と `results.json` は、VoiceOver の実機実行後に静的状態ラベルを変更したことを明記している。最終コード `91822ca` の VoiceOver 一連操作を実行済みとは読めない。
- `target/issue-55-independent-1/noncurrent-image/launch-1.json` は最終コード `91822ca` を指し、画像診断の独立確認は成功している。無効入力の独立確認も `target/issue-55-independent-1.log` で成功している。
- `target/issue-55-ime-flow-1.log` の独立 IME 実行は、変換候補ダイアログ表示中の状態判定で失敗している。製品の保存・取消経路まで進んだ証拠ではなく、最終コードの日本語 IME 受け入れ確認は未完了である。
