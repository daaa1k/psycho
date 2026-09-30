# Issue #18 macOS / GPUI 検証結果

## 判定

ユーザーの指摘を受けて AeroSpace を停止し、Orca を再起動した後に編集 2 サイズと全画面の配置、発表操作を取り直した。
共有レイアウト、はみ出し停止と修正、全画面の移動と復帰は確認できた。
標準日本語 IME の候補選択・確定と変換単位の Undo / Redo も実機で一度確認したが、その際に選択範囲計算の panic が見つかった。
選択範囲の計算を修正し、回帰テストを追加した。修正版での IME 再試験、再変換、変換中の保存と発表、物理ピクセル誤差は未確認である。
必須ケースの未解決が残るため、Issue #18 の合格とは判定しない。

## 環境

| 項目 | 観測値 |
| --- | --- |
| Mac | MacBook Air、Apple M3、arm64 |
| macOS | 26.5.2、Build 25F84 |
| Xcode | 26.5、Build 17F42 |
| Rust | 1.98.1 |
| GPUI / GPUI platform | Zed commit `14dd03e89676e7fe74bc205001bfb32c8cfc3952` |
| 表示中の画面 | 内蔵 Liquid Retina 2560 × 1664 Retina。今回の `system_profiler` 取得では内蔵画面のみ |
| GPUI window scale | 1 |
| 小さい編集表示 | viewport 1024 × 760、ウィンドウ 1024 × 792 |
| 大きい編集表示 | viewport 1440 × 960、ウィンドウ 1440 × 992 |
| 全画面 | viewport とウィンドウが 1920 × 1200 |
| 配置倍率 | 小 0.7556、大 1.0333、全画面 1.5 |
| 日本語フォント | Hiragino Sans |
| コードフォント | Menlo |
| 代替フォント | 個別指定なし。OS / GPUI のフォールバック先は未測定 |

AeroSpace が動作していた初回記録では、「小さい編集表示」のままウィンドウが 1024 × 792、1472 × 965、1890 × 1169 に変化した。
ユーザーの指摘を受けて AeroSpace を停止し、Orca 再起動後に各サイズを再測定した。
取り直した記録では小 1024 × 792、大 1440 × 992、全画面 1920 × 1200 になった。
今回の `system_profiler` 取得では内蔵ディスプレイだけが表示された。

## 結果表

| ID | 期待結果 | 観測結果 | 判定 |
| --- | --- | --- | --- |
| ENV-01 | Apple Silicon、単一画面、OS / GPUI revision、使用フォントと代替フォント、倍率、表示サイズ、再実行手順を記録する | OS、GPUI revision、使用フォント、倍率、画面サイズ、再実行手順を記録した。今回の画面取得は内蔵画面のみだが、代替フォントとフォールバック先は未測定 | 一部記録 |
| BUILD-01 | プローブをビルドして起動できる | `cargo check` と `cargo run` が成功した。選択範囲計算の回帰テスト 2 件も成功した | 合格 |
| IME-01 | Kotoeri が日本語の未確定文字列を表示する | Kotoeri で `nihongohenkan` をキー入力し、下線付きの未確定文字列を確認した。選択範囲修正後の再試験待ち | 修正前に確認 |
| IME-02 | 変換候補を表示して選択し、確定できる | 候補一覧に「日本語変換」が現れ、候補を選択して Return で「日本語変換」を確定した。修正前の画面証跡あり。修正版で再試験待ち | 修正前に確認 |
| IME-03 | 候補ウィンドウを入力位置に置き、物理座標誤差を 1 px 以内にする | 候補ポップアップが未確定文字列の直下に表示された。GPUI の候補範囲ログもあるが、物理ピクセル差は未計測。修正版で再試験待ち | 一部確認 |
| IME-04 | 再変換、Enter / Escape、二重 action の有無を確認する | 候補確定時に Enter action カウンターは 0 で、二重 action は観測しなかった。再変換と変換中 Escape は未確認。修正版で再試験待ち | 一部確認 |
| IME-05 | 変換単位の Undo / Redo と変換中の保存と発表を確認する | Cmd+Z 1 回で変換前のタイトルへ戻り、Cmd+Shift+Z 1 回で確定文字列が復帰した。Redo 後に古い選択範囲計算で panic が発生したため修正し、回帰テストを追加した。変換中の保存と発表、修正版での再試験は未完了 | 修正前に一部確認 |
| IME-06 | 無効入力を修正または取消し、保存と発表を再開できる | 空のタイトルで保存と発表が止まり、タイトル修正と取消後に続行できた。IME 変換中の無効入力では試していない | 一部確認 |
| LAYOUT-01 | 日本語、英語、URL、箇条書き、タブ入りコード、Caption を同じ配置で表示する | 1:1 と 1:2 の小さい編集表示、大きい編集表示、全画面で画面を取得した。混在本文、長い URL、箇条書き、タブ入りコード、画像枠、Caption を確認した | 合格 |
| LAYOUT-02 | 2 つの編集サイズと全画面で改行と基準配置が一致し、描画丸め誤差が物理 1 px 以内になる | 3 サイズで同じ base 座標を記録し、スクリーンショットでは URL の改行位置も一致した。物理ラスタの丸め誤差を画像から個別測定していない | 一部確認 |
| LAYOUT-03 | 1:2 の列、はみ出し診断、発表停止、修正後の再開を確認する | 1:2 を確認した。はみ出し Slide では発表開始が止まり、修正後は現在位置から開始できた。診断は要素枠を調べ、枠内文字のはみ出しは検出しない | 合格 |
| PRESENT-01 | 最初と現在の Slide から発表を開始できる | 先頭と現在の Slide の両方から開始した | 合格 |
| PRESENT-02 | 全移動キー、先頭と末尾の停止、Escape 後のフォーカス復帰を確認する | Right、Down、Space、PageDown、Left、Up、PageUp を試した。先頭の Left と末尾の Right は停止し、Escape 後に編集ウィンドウへ戻った | 合格 |
| PRESENT-03 | 開始と終了を 10 回繰り返し、別アプリへ切り替えて復帰する | 10 回の開始と終了を完了した。記録画面の全画面遷移カウンターは 26 だった。別アプリへの切替後に戻り、次の Slide へ進めた | 合格 |
| VIDEO-01 | IME と全画面の録画を残す | 15 秒、1920 × 1200 の全画面ウィンドウ録画を作成した。IME の画面証跡はあるが動画は未作成 | 一部記録 |

## 観測の詳細

AeroSpace を停止した後の座標ログでは、小さい編集表示が 1024 × 792、大きい編集表示が 1440 × 992、全画面が 1920 × 1200 になった。
同じ Slide の `base_x`、`base_y`、`base_width`、`base_height` は各表示モードで一致した。
ログに記録した座標は GPUI の配置値であり、スクリーンショットの物理ピクセル境界を測定した値ではない。
そのため、1 px 許容条件はまだ判定していない。

Kotoeri の候補ポップアップ、確定後のタイトル、Undo 後と Redo 後の画面を修正前の実機で記録した。
確定と Undo / Redo の後、選択範囲を置換後の文書全体へ誤って適用していたため、`start byte index 10 is out of bounds for string of length 9` でプローブが panic した。
Apple の [`NSTextInputClient.setMarkedText` 仕様](https://developer.apple.com/documentation/appkit/nstextinputclient/setmarkedtext%28_%3Aselectedrange%3Areplacementrange%3A%29)では `selectedRange` は挿入文字列の先頭から計算する。実装を新しい変換文字列内で UTF-16 から UTF-8 へ変換するよう修正し、旧式で失敗する2件の回帰テストが修正後に成功することを確認した。
修正版での実機再試験のためプローブを再起動したが、Orca Computer Use は対象ウィンドウを列挙できる一方、「visible windows but no accessibility window」として AX 操作を拒否した。`orca computer permissions` は Accessibility を granted と報告している。修正版での IME 再試験は保留中。
試験のため一時的に有効化した Kotoeri は無効化し、入力ソースを元の ABC に戻した。TIS API の両操作は OSStatus 0 で完了し、システム設定でも ABC が選択中であることを確認した。

通常テキストの Undo / Redo と空タイトルの修正と取消は確認した。
これらは IME の変換単位動作を確認した証拠には数えない。

発表画面では Right、Down、Space、PageDown で次へ進み、Left、Up、PageUp で前へ戻った。
Slide の先頭と末尾で移動が止まり、Escape で編集表示へ戻った。
10 回の開始と終了後も編集表示は 1024 × 792 に戻り、別アプリへ切り替えた後に発表位置を維持して操作を続けられた。

## 証跡

### 配置比較

- [小さい編集表示、比率 1:1](evidence/layout-small-1-1-live.png)
- [小さい編集表示、比率 1:2](evidence/layout-small-1-2-live.png)
- [大きい編集表示、比率 1:1](evidence/layout-large-1-1-live.png)
- [大きい編集表示、比率 1:2](evidence/layout-large-1-2-live.png)
- [全画面、比率 1:1](evidence/layout-fullscreen-1-1-live.png)
- [全画面、比率 1:2](evidence/layout-fullscreen-1-2-live.png)
- [AeroSpace 無効時の配置座標](evidence/layout-coordinate-log-aerospace-disabled.csv)
- [AeroSpace 有効時の寸法履歴](evidence/layout-window-dimensions-aerospace-enabled.csv)

### IME と入力復旧

- [IME 未確定文字列](evidence/ime-marked-composition.png)
- [候補一覧の AX 取得](evidence/ime-candidate-accessibility.txt)
- [候補ポップアップと未確定文字列](evidence/ime-candidate-region-live.png)
- [候補確定後](evidence/ime-commit-live.png)
- [変換単位の Undo 後](evidence/ime-undo-live.png)
- [変換単位の Redo 後](evidence/ime-redo-live.png)
- [候補範囲の座標ログ](evidence/ime-candidate-coordinate-log.csv)
- [空タイトルによる保存と発表の停止](evidence/presentation-invalid-blocked-live.png)
- [タイトル修正後の復旧](evidence/presentation-invalid-recovered-live.png)

### 発表操作

- [はみ出し検出中の全画面](evidence/layout-fullscreen-overflow-live.png)
- [Slide 3 のはみ出し修正後](evidence/layout-fullscreen-slide3-fixed-live.png)
- [現在位置からの発表復帰](evidence/presentation-return-slide3-live.png)
- [別アプリ切替後の発表復帰](evidence/presentation-app-switch-return-live.png)
- [10 回の開始と終了後](evidence/presentation-10-cycles-live.png)
- [全画面録画](evidence/presentation-fullscreen-window-live.mov)

AeroSpace 有効時のセッション履歴は [配置ログの履歴](evidence/layout-coordinate-log-session-history.csv) に残した。
現在の受け入れ判定には、寸法が確定した AeroSpace 無効時のログを使った。

## 検証コマンド

```sh
~/.cargo/bin/cargo fmt --manifest-path validation/macos-gpui-probe/Cargo.toml --check
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ ~/.cargo/bin/cargo check --manifest-path validation/macos-gpui-probe/Cargo.toml
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ ~/.cargo/bin/cargo test --manifest-path validation/macos-gpui-probe/Cargo.toml
```

3 コマンドは成功した。
Cargo は依存クレート `block 0.1.6` の将来互換性警告を出した。
