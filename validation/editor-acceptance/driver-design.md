# 実機入力の検証境界

受け入れスクリプトは、実際の入力欄、メニュー、キーボードから操作する。製品の内部状態を直接変更しない。

初回の代表例と白紙作成は入力欄のフォーカス取得で停止した。文書の確認ダイアログ、全画面、表示ポップアップからの Tab と Right は、Orca の `window_not_focused` で停止した。これらは製品機能の合格証拠に数えず、失敗ログを保持する。

## 比較した形

| 候補 | 構造 | 判断 |
| --- | --- | --- |
| gpt-6-luna | 共有 `Gui` から既存の AX 読取、ダブルクリック、Escape ヘルパーを組み合わせる | 最小差分。ただし AXWindow と別取得した CGWindow の原点が一致する前提が残り、一般的な focused フラグだけでは対象欄を特定できない |
| gpt-6-sol | 共有 `Gui` の操作を小さな Swift ヘルパーへ集約する | 採用。対象欄の座標、焦点、プロセスの照合を同じ境界で行う |
| gpt-6-astra | 同様の専用ヘルパーに、対象 AXWindow の特定、再送禁止、詳細な操作記録を持たせる | 対象 AXWindow と再送禁止を採用。全操作共通の詳細記録は増やさない |

独立した gpt-6-astra の比較も sol 案を推奨した。比較後、Escape 以外の Tab と Right でも同じ検証境界が停止したため、キー操作は `key CHORD` にまとめた。保存メニューへの置換は行わず、Cmd+S も実際の修飾キーとハードウェアキーとして送る。

## 呼び出しと責務

`Gui.focus_input(label)` は完全一致する入力欄を指定する。Swift は所有する PID と実行ファイルの絶対パス、前面アプリ、AX 権限を検査する。欄が所属する AXWindow と欄のグローバル座標から中央を1回クリックし、送信後に同じ欄の AX フォーカスを確認する。Python も製品の入力状態が focused であることを確認する。

`Gui.key(chord)` と `Gui.hotkey(chord)` は所有アプリが前面であることを確認し、修飾キーの押下、実キーの押下と解放、修飾キーの解放を OS の HID イベントとして送る。VoiceOver の操作には修飾フラグに加えて flagsChanged イベントを送る。各イベントの後も送信プロセスを短時間維持する。PID への直接配送を使った最初の代表例では Cmd+V 後もコードが置換されなかったため、通常の OS 入力経路へ変更した。配送結果を `native-input.jsonl`、ヘルパーのビルドハッシュを `native-input-build.json` に記録する。再送しない。入力値、保存した KDL、履歴、選択、モーダルの取消、全画面からの復帰は各レシピが検査する。配送の完了や待機時間だけで製品の合格とはしない。

`Gui.replace(text)` は選択した入力欄へ Cmd+A と Cmd+V を送り、製品の入力値が期待する文字列と一致するまで確認する。空文字列は Backspace で削除する。IME の検証ではこの置換を使わず、標準入力ソースで変換を行う。

`ime_save_acceptance.py --entry shortcut` の明示的な Orca 操作は、既存 #77 の同じ再現経路として残す。ネイティブメニューでの保存と区別して判定する。

## 適用した原則

Prove It Works に従い、操作の返値に加え、製品状態と保存ファイルを照合する。Model the Domain と Type System Discipline に従い、Swift の `Operation` と `Chord` で欄への焦点移動と実キーを表現する。Boundary Discipline に従い、OS の対象と入力形式の検査を Swift 境界へ集める。

Laziness Protocol に従い、既存 `Gui` の呼び出しを維持し、レシピごとの座標変換を増やさない。Foundational Thinking と Sequence Work into Verifiable Units に従い、前提の統合、入力境界、実際の通し操作の順に検証する。Attack the Premise に従い、フォーカス取得の再試行を増やす前に、停止したケースを境界別に記録した。
