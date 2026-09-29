# Issue #18 macOS / GPUI 検証結果

## 判定

検証は未完了である。
ビルドとプローブの起動を確認した。
Kotoeri の未確定文字列と候補一覧を確認した。
候補確定、再変換、Undo / Redo、保存と発表、二つの編集サイズ、全画面操作は完了していない。
Issue #18 の合格とは判定しない。

## 環境

| 項目 | 観測値 |
| --- | --- |
| Mac | MacBook Air、Apple M3、arm64 |
| macOS | 26.5.2、Build 25F84 |
| Xcode | 26.5、Build 17F42 |
| Rust | 1.98.1 |
| GPUI / GPUI platform | Zed commit `14dd03e89676e7fe74bc205001bfb32c8cfc3952` |
| IME | macOS 標準 Kotoeri、RomajiTyping、Hiragana |
| フォント | Hiragino Sans、Menlo |
| 代替フォント | 個別指定なし。OS / GPUI のフォールバック先は未測定 |
| GPUI ウィンドウ | 再開時の画面取得で 1890 × 1169 px、viewport 1890 × 1137 px、1x を観測 |
| 内蔵ディスプレイ | 2560 × 1664 Retina |
| 編集ウィンドウ指定サイズ | 小 1024 × 760 px、大 1440 × 960 px |

検証途中の `NSScreen` と `system_profiler` は BenQ GW2765 外部ディスプレイ 2560 × 1440 も示した。
後の `system_profiler` では内蔵ディスプレイだけが表示された。
画面構成が切り替わった時刻は記録していない。
したがって単一ディスプレイ条件を通した比較結果はない。

## 結果表

| ID | 期待結果 | 観測結果 | 判定 |
| --- | --- | --- | --- |
| ENV-01 | Apple Silicon、単一画面、OS / GPUI revision、使用・代替フォント、倍率、表示サイズ、再実行手順を記録する | MacBook Air / Apple M3、OS と GPUI revision、使用フォント、内蔵画面、コード指定の編集サイズ、再実行手順を記録した。外部画面の検出履歴があり、代替フォントとフォールバック先は未測定 | 一部記録 |
| BUILD-01 | プローブをビルドして起動できる | `cargo check` と `cargo run` が成功した | 合格 |
| IME-01 | Kotoeri が日本語の未確定文字列を表示する | `nihongohenkan` を一打ずつ送り、未確定範囲と入力欄の表示を確認した | 一部観測 |
| IME-02 | 変換候補を表示して選択し、確定できる | AX 候補一覧に「日本語変換」などが現れた。候補選択と確定は未実施 | 未完了 |
| IME-03 | 候補ウィンドウを入力位置に置き、物理座標誤差を 1 px 以内にする | GPUI が IME に返した候補範囲をログに記録した。候補ウィンドウの位置は画面で確認できていない | 未確認 |
| IME-04 | 再変換、Enter / Escape、二重 action の有無を確認する | 再変換、候補確定、取消を完了していない | 未完了 |
| IME-05 | 確定単位の Undo / Redo と変換中の保存・発表を確認する | 操作を完了していない | 未完了 |
| IME-06 | 無効入力を修正または取消し、保存・発表を再開できる | 操作を完了していない | 未完了 |
| LAYOUT-01 | 日本語、英語、URL、箇条書き、タブ入りコード、Caption を同じ配置で表示する | 小さい編集表示で 1:1 と 1:2 の両画面を撮影した。1:1 では混在本文、折返し URL、箇条書き、タブ入りコード、画像枠、Caption を確認した。1:2 では英語本文、左右列、Caption を確認した | 一部観測 |
| LAYOUT-02 | 二つの編集サイズと全画面で折返しと列比率を比較し、誤差を 1 px 以内にする | 1:1 の配置比較画像と座標ログは残した。小さい編集表示の初期描画中に viewport が 1024 × 760、1472 × 933、1890 × 1137 px と変化した。大きい編集表示と全画面の比較、改行比較は未実施 | 未完了 |
| LAYOUT-03 | 1:2 の列、はみ出し診断、発表停止、修正後の再開を確認する | 操作を完了していない。診断コードは余白から出た要素枠を検出するが、枠内の文字あふれは検出しない | 未完了 |
| PRESENT-01 | 最初と現在の Slide から発表を開始できる | 操作を完了していない | 未完了 |
| PRESENT-02 | 前後移動、先頭・末尾の停止、フォーカス復帰を確認する | 操作を完了していない | 未完了 |
| PRESENT-03 | 開始と終了を 10 回繰り返し、別アプリへ切り替えて復帰する | 操作を完了していない | 未完了 |

## 観測の詳細

小さい編集表示のスクリーンショットには、日本語と英語の本文、長い URL、箇条書き、タブ入り Rust コード、1:1 画像枠、Caption が写っている。
最初の画面では編集表示内のキャンバスがツールバー高分だけ二重に下がっていた。
描画位置から重複分を除き、再取得した画面では Caption まで見えるようになった。
この修正はウィンドウ内座標の丸め誤差を比較した結果ではない。

アクセシビリティ権限を切り替えた後、小さい編集表示の 1:1 と 1:2 を撮影した。
1:2 の画面では左列の箇条書き、右列のコードと画像枠、画像下の Caption が見える。
同じ再開セッションの座標ログでは、モードが `editing-small` のまま viewport とウィンドウ寸法が 3 回変化した。
その変化を起こした要因は特定できていないため、編集サイズの比較結果として扱わない。

IME にはキーイベントで入力した。
`type-text` は IME を経由せず文字列を直接挿入したため、IME 検証には使わなかった。
Kotoeri の AX 候補一覧には「日本語変換」が含まれていた。
候補位置ログには未確定範囲と論理・物理座標が残っている。
候補一覧が出ていたときの画面取得には候補パネル自体が写っていない。

## 証跡

- [小さい編集表示・1:1](evidence/layout-small-1-1.png)
- [再開時の小さい編集表示・1:1](evidence/layout-small-1-1-live.png)
- [小さい編集表示・1:2](evidence/layout-small-1-2-live.png)
- [IME 未確定文字列](evidence/ime-marked-composition.png)
- [候補一覧の AX 取得](evidence/ime-candidate-accessibility.txt)
- [配置座標ログ](evidence/layout-coordinate-log.csv)
- [再開時のウィンドウ寸法ログ](evidence/layout-window-dimensions-live.csv)
- [候補範囲座標ログ](evidence/ime-candidate-coordinate-log.csv)

Issue #18 が求める IME と全画面の動画記録は作成できていない。
動画を含む必須ケースの結果は未完了のまま残した。

## 中断理由と再開条件

権限切替後は `get-app-state` と画面取得に成功したが、その後の操作で `permission_denied` が再発した。
`orca computer permissions --id accessibility --json` は Accessibility と Screenshots を `granted` と報告し、Computer Use helper を起動した。
helper 起動後の `get-app-state` も、対象アプリの Accessibility 読み取りが 1500 ms の再試行後に失敗した。
Orca が示した helper の場所は `/Applications/Orca.app/Contents/Resources/Orca Computer Use.app` である。

「アクセシビリティ」一覧で上記の Computer Use helper の登録状態を確認し、必要ならその項目をオフ・オンした後に Orca を再起動する。
その後、この README の起動手順を使ってケース表の未完了行を再実行する。
すべての必須行に実測値と証跡が揃うまで、この検証を合格にしない。

## 検証コマンド

```sh
~/.cargo/bin/cargo fmt --manifest-path validation/macos-gpui-probe/Cargo.toml --check
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ ~/.cargo/bin/cargo check --manifest-path validation/macos-gpui-probe/Cargo.toml
```

上記二つのコマンドは成功した。
Cargo は依存クレート `block 0.1.6` の将来互換性警告を出した。
