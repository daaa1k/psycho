# Issue #18 macOS / GPUI 検証結果

## 判定

Issue #18 の実機先行検証は合格と判定する。macOS / GPUI で製品実装へ進める。
IMEの入力、候補、再変換、保存、発表、Undo / Redoと全画面の移行・復帰を確認した。
各Return直後の画面、描画文字列、IMEコールバック、保存値を100回検査し、欠落や重複はなかった。
描画修正後も同じ検査を10回、全移動キーを含む全画面開始・終了を10回実行した。

文字配置の独立測定で、タブ位置の拡縮不一致と行高の丸め累積を見つけた。
基準サイズで字形と改行を決め、グリフ位置、行高、コード余白を一緒に拡縮するよう修正した。
コードのタブは表示時だけ4文字のタブストップへ展開し、コード行は折り返さない。
CoreTextの基準位置と全12要素・1,473グリフを比較した結果、描画丸め後の各軸誤差は最大0.494689物理pxだった。
全キャンバスと各グリフの画像検査でも、欠けや余分な描画は検出しなかった。

過去に一度観測した二重表示は再現しなかった。
利用者は検証中にPCを操作していた可能性を申告し、再現しない観測は別課題に残して先へ進むよう指示した。
この指示に従い、原因未特定の観測を[Issue #31](https://github.com/daaa1k/psycho/issues/31)へ移した。
操作干渉が原因とも、入力実装の不具合が修正済みとも判定していない。

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
| 代替フォント | 個別指定なし。CoreText で Menlo の日本語を `HiraginoSans-W3`、絵文字を `AppleColorEmoji` に解決。GPUIのフォント名は直接取得せず、全グリフのID・位置・ラスタをCoreText参照と照合 |

AeroSpace が動作していた初回記録では、「小さい編集表示」のままウィンドウが 1024 × 792、1472 × 965、1890 × 1169 に変化した。
ユーザーの指摘を受けて AeroSpace を停止し、Orca 再起動後に各サイズを再測定した。
取り直した記録では小 1024 × 792、大 1440 × 992、全画面 1920 × 1200 になった。
今回の `system_profiler` 取得では内蔵ディスプレイだけが表示された。

## 結果表

| ID | 期待結果 | 観測結果 | 判定 |
| --- | --- | --- | --- |
| ENV-01 | Apple Silicon、単一画面、OS / GPUI revision、使用フォントと代替フォント、倍率、表示サイズ、再実行手順を記録する | OS、GPUI revision、使用フォント、倍率、画面サイズ、再実行手順を記録した。代替フォントは個別指定せず、CoreText のフォールバック先を記録した | 合格 |
| BUILD-01 | プローブをビルドして起動できる | `cargo check` と `cargo run` が成功した。選択範囲計算の回帰テスト2件とタブストップのテスト1件も成功した | 合格 |
| IME-01 | Kotoeri が日本語の未確定文字列を表示する | 修正版でキー入力による下線付き未確定文字列を再確認した | 合格 |
| IME-02 | 変換候補を表示して選択し、確定できる | 修正版で候補一覧から「日本語変換」を確定し、保存ファイルにも同じ文字列を確認した | 合格 |
| IME-03 | 候補ウィンドウが入力位置へ追従する | 先頭の「日本語」とその後の「変換」で候補ポップアップを撮影した。入力範囲の左端は画面座標 525.0 px から 633.8 px、候補枠の左端は 516 px から 624 px へ移動した。移動量の差は 0.8 px | 合格 |
| IME-04 | 再変換、Enter / Escape、文字の欠落と二重入力、二重 action の有無を確認する | 再変換、Return 2回、Escape 2回、変換中のactionカウンターを確認した。100回の確定直後画像OCR・描画値・コールバック列・保存値と修正版の追加10回で欠落や重複なし。未再現の過去の二重表示は利用者の指示でIssue #31へ移した | 合格 |
| IME-05 | 変換単位の Undo / Redo と変換中の保存と発表を確認する | 未確定の「日本語変換」を保存操作で確定し、保存ファイルに同じ値を確認した。次の「桜」は発表操作で確定して全画面に表示され、保存ファイルは「日本語変換」のままだった。Cmd+Z / Cmd+Shift+Z で両変換値を 1 単位で往復できた | 合格 |
| IME-06 | 無効入力を修正または取消し、保存と発表を再開できる | 未確定文字列を Escape 2 回で消して空タイトルにした後、「入力取消」で保存済みのタイトルへ復帰した。未確定の「日本語変換」を入力取消した直後に「桜」を入力でき、古い候補は戻らなかった | 合格 |
| LAYOUT-01 | 日本語、英語、URL、箇条書き、タブ入りコード、Caption を同じ配置で表示する | 1:1 と 1:2 の小さい編集表示、大きい編集表示、全画面で画面を取得した。混在本文、長い URL、箇条書き、タブ入りコード、画像枠、Caption を確認した | 合格 |
| LAYOUT-02 | 2つの編集サイズと全画面で改行と基準配置が一致し、描画丸め誤差が物理1px以内になる | 基準サイズのCoreText参照と1:1 / 1:2の全12要素・全1,473グリフを比較した。グリフID・数・順序、改行と位置が一致し、描画丸め後の各軸誤差は最大0.494689px。全キャンバスと各グリフの双方向画像検査でも欠けや余分な描画なし | 合格 |
| LAYOUT-03 | 1:2 の列、はみ出し診断、発表停止、修正後の再開を確認する | 1:2 を確認した。はみ出し Slide では発表開始が止まり、修正後は現在位置から開始できた。診断は要素枠を調べ、枠内文字のはみ出しは検出しない | 合格 |
| PRESENT-01 | 最初と現在の Slide から発表を開始できる | 先頭と現在の Slide の両方から開始した | 合格 |
| PRESENT-02 | 全移動キー、先頭と末尾の停止、Escape 後のフォーカス復帰を確認する | Right、Down、Space、PageDown、Left、Up、PageUp を試した。先頭の Left と末尾の Right は停止し、Escape 後に編集ウィンドウへ戻った | 合格 |
| PRESENT-03 | 開始と終了を 10 回繰り返し、別アプリへ切り替えて復帰する | 10 回の開始と終了を完了した。記録画面の全画面遷移カウンターは 26 だった。別アプリへの切替後に戻り、次の Slide へ進めた | 合格 |
| VIDEO-01 | IME と全画面の録画を残す | 15 秒、1920 × 1200 の全画面録画と 15 秒、1024 × 792 の IME 入力録画を保存した | 合格 |

## 観測の詳細

AeroSpace を停止した後の座標ログでは、小さい編集表示が 1024 × 792、大きい編集表示が 1440 × 992、全画面が 1920 × 1200 になった。
同じ Slide の `base_x`、`base_y`、`base_width`、`base_height` は各表示モードで一致した。
ログに記録した座標は GPUI の配置値である。
保存済みスクリーンショットから画像枠とコード枠の背景色を検出し、編集画面ではウィンドウ上端の 32 px を加えたログ上の座標と比較した。
画像枠の 1 px の枠線を考慮した 48 辺の誤差は最大 0.778 px だった。
今回、文字を含む 9 要素について、最初の暗い画素の位置を 3 サイズの保存画像から取得した。
各要素の先頭画素から共有レイアウト上の相対位置を推定し、サイズごとの残差を計算すると最大 0.916 物理 px だった。
Slide 1 の箇条書きは直前の URL と測定枠が接するため、この測定から除外した。
この先頭画素の測定は相対位置の追従だけを示す。最終判定には後述する全グリフの独立測定を使った。

Kotoeri の候補ポップアップ、確定後のタイトル、Undo 後と Redo 後の画面を修正前の実機で記録した。
確定と Undo / Redo の後、選択範囲を置換後の文書全体へ誤って適用していたため、`start byte index 10 is out of bounds for string of length 9` でプローブが panic した。
Apple の [`NSTextInputClient.setMarkedText` 仕様](https://developer.apple.com/documentation/appkit/nstextinputclient/setmarkedtext%28_%3Aselectedrange%3Areplacementrange%3A%29)では `selectedRange` は挿入文字列の先頭から計算する。実装を新しい変換文字列内で UTF-16 から UTF-8 へ変換するよう修正し、旧式で失敗する2件の回帰テストが修正後に成功することを確認した。
今回は Orca Computer Use でプローブの Accessibility ウィンドウと画面を取得できた。
修正版で未確定入力、候補表示と確定、Ctrl+Shift+R による再変換、Undo / Redo を再試験した。
最初の修正版では未確定中の保存と発表がそれぞれ停止し、手動で確定した後は実行できた。
親仕様 [#16 の決定](https://github.com/daaa1k/psycho/issues/16#issuecomment-5885248499)は保存と発表開始時に IME 変換を自動確定することを求める。
そこで未確定範囲だけを消す実験をしたが、Kotoeri の変換セッションが残り、次の入力で古い候補が表示された。
現在のプローブは変換文字列を Undo に確定し、AppKit の [`NSTextInputContext.discardMarkedText`](https://developer.apple.com/documentation/appkit/nstextinputcontext/discardmarkedtext%28%29) で変換セッションを終了する。
保存と発表だけでなく、入力取消とタイトルの置換にも同じ処理を適用した。
実機では未確定の「日本語変換」を保存した後、次の「桜」の入力に古い候補は現れなかった。
「桜」を未確定のまま発表すると全画面には「桜」が表示され、保存ファイルには以前の「日本語変換」が残った。
発表から戻り、Cmd+Z で「日本語変換」、Cmd+Shift+Z で「桜」に戻った。
再変換の確定には Return が 2 回必要で、Enter action カウンターは増えなかった。
未確定中の Escape は 1 回目では文字列が残り、2 回目で空になった。
「入力取消」で保存済みタイトルに戻した。未確定中の取消後にも新しい「桜」を入力できた。
別の再入力では「日本語変換日本語変換」が一度表示されたが、同じキー列の再実行 3 回では保存値がすべて「日本語変換」だった。
今回も「にほんごへんかん」、Space、Return 2 回を 10 回繰り返し、毎回保存した。
10 回とも保存値は「日本語変換」で、最後の画面にも二重表示はなかった。
その後、確定直後を検査する100回のループと描画修正後の10回でも再現しなかった。利用者の指示に従い、原因未特定の観測をIssue #31に残した。
候補位置は、タイトル先頭で「日本語」を変換した後、続く位置で「変換」を変換して測った。
座標ログの入力範囲左端はウィンドウ内で 77.0 px と 185.8 px、ウィンドウ左端は画面座標 448 px だった。
候補枠の左端は画面座標 516 px と 624 px で、移動量は入力範囲が 108.8 px、候補枠が 108 px だった。
候補枠の測定は画面画像の上端から 520 px の走査線で行った。
試験後は入力ソースを ABC に戻し、Kotoeri の親入力ソースを開始時と同じ無効状態に戻した。TIS API はどちらも OSStatus 0 を返した。

フォントの確認には CoreText を使った。
Hiragino Sans は `HiraginoSans-W3`、Menlo は `Menlo-Regular` に解決された。
Menlo の日本語は `HiraginoSans-W3`、絵文字は `AppleColorEmoji` にフォールバックした。
GPUI の macOS 実装は CoreText のシステムフォールバック一覧を利用するが、今回の測定は GPUI が描いた個別グリフのフォントを直接取得したものではない。

今回、全グリフの独立測定とIMEの反復検査を完了した。二重表示の原因調査はIssue #31で追跡する。

通常テキストの Undo / Redo と空タイトルの修正と取消は確認した。
これらは IME の変換単位動作を確認した証拠には数えない。

発表画面では Right、Down、Space、PageDown で次へ進み、Left、Up、PageUp で前へ戻った。
Slide の先頭と末尾で移動が止まり、Escape で編集表示へ戻った。
10 回の開始と終了後も編集表示は 1024 × 792 に戻り、別アプリへ切り替えた後に発表位置を維持して操作を続けられた。

## 最終検査の詳細

### 全グリフの位置とラスタ

独立したCoreText参照は、HiraginoSans-W3、見出しのHiraginoSans-W6、Menlo-Regularを使い、基準サイズで位置を計算する。
Menloの日本語グリフはHiraginoSans-W3へフォールバックした。
`layout-reference.json` に固定した文字列、改行、座標、フォントから参照値を作り、GPUIの配置値や実画像から参照位置を補正しない。

最初の比較では、CoreTextの既定タブ送りが表示倍率によらず28pxであるため、拡大時のコード位置が基準配置とずれた。
複数行の行高を毎回丸める方法にも累積差があった。
基準サイズでの字形と改行に変更した後も、コード枠と内側余白の別々の丸めによって、拡大編集表示のコード2行目の51グリフが最大1.056582pxずれた。
この中間版は保存済みログと画像から測定コマンドを実行すると不合格になる。
完成版はGPUIの要素枠の丸めを文字原点へ引き継がず、共通配置の原点から直接描画する。

全12要素の空白以外の491グリフを3サイズで測り、1,473件すべてが合格した。
空白の送りは後続文字の座標で検査した。
GPUIに渡した位置を同リビジョンの描画規則どおりxは1/4px、yは1pxへ丸めた座標と、CoreTextの参照座標を比較した。
最大各軸誤差は0.494689pxで、許容範囲の物理1px以内だった。
さらに各グリフ領域と全キャンバスで、濃い画素が相手画像の薄い画素から各軸1px以内にあることを双方向で確認した。
濃淡の中間域はアンチエイリアス差として除外した。閾値と再実行手順はREADMEに記録した。
この判定は保存した文字列、フォント、画面倍率と表示サイズを対象とし、任意のフォントや画面構成での保証ではない。

### IMEの反復と最終操作

既存の二重表示画像はOCRが「日本語変換日本語変換」と読み取った。
`repeat_ime.py` は標準Kotoeriへ「にほんごへんかん」、Space、Return 2回を送り、各Return直後の画像OCRと描画文字列、その間の全更新コールバックを検査する。
毎回保存し、10回ごとに確定文字列の再変換も行った。
100回とも値は「日本語変換」で、重複・欠落を検出しなかった。
字形の共通配置へ変更した完成版でも、同じループを10回実行してすべて合格した。

完成版で未確定の「日本語変換」を保存し、ファイルと描画に同じ値を確認した。
未確定の「桜」から発表を開始すると、発表画面は「桜」、保存ファイルは「日本語変換」のままだった。
発表終了後のUndoで「日本語変換」、Redoで「桜」に戻り、変換単位の履歴を再確認した。
全移動キーを含む全画面開始・終了も10回繰り返し、毎回1024×792の編集表示へ戻った。

今回の開始時にはazooKeyが選択され、AeroSpaceは有効だった。
試験中だけ標準Kotoeriを選び、AeroSpaceの配置制御を無効にした。
終了後はazooKeyを選択し、Kotoeriの親ソースを再び無効化した。TISの戻り値はいずれもOSStatus 0で、一覧の選択・有効状態も開始時と一致した。
AeroSpaceも再び有効にし、プローブを終了した。

## 証跡

### 最終検査

- [小さい編集表示、1:1](evidence/shared-layout/layout-small-1-1-live.png)
- [小さい編集表示、1:2](evidence/shared-layout/layout-small-1-2-live.png)
- [大きい編集表示、1:1](evidence/shared-layout/layout-large-1-1-live.png)
- [大きい編集表示、1:2](evidence/shared-layout/layout-large-1-2-live.png)
- [全画面、1:1](evidence/shared-layout/layout-fullscreen-1-1-live.png)
- [全画面、1:2](evidence/shared-layout/layout-fullscreen-1-2-live.png)
- [共通配置の座標ログ](evidence/shared-layout/layout-coordinates.csv)
- [描画グリフの原点ログ](evidence/shared-layout/glyph-origins.csv)
- [独立したCoreTextの参照座標](evidence/shared-layout/glyphs.json)
- [全1,473グリフの測定結果](evidence/shared-layout/glyph-measurements.csv)
- [中間版の1px超過を検出した測定値](evidence/intermediate-rounding/glyph-measurements.csv)
- [中間版の座標ログ](evidence/intermediate-rounding/glyph-origins.csv)
- [中間版の大きい編集表示、1:2](evidence/intermediate-rounding/layout-large-1-2-live.png)
- [100回の確定直後検査値](evidence/ime-repeat-100-checks.csv)
- [全200枚の確定直後画像と10枚の再変換画像](evidence/ime-repeat-100-snapshots.zip)
- [100回検査と直前の試走10回の更新コールバック列](evidence/ime-repeat-100-events.tsv.gz)
- [100回目の確定直後](evidence/ime-repeat-100-final.png)
- [完成版の追加10回の検査値](evidence/ime-final-10-checks.csv)
- [完成版の10回目の確定直後](evidence/ime-final-10-live.png)
- [完成版で未確定文字列を保存](evidence/ime-shared-save-live.png)
- [完成版で未確定文字列から発表](evidence/ime-shared-presentation-live.png)
- [完成版のUndo](evidence/ime-shared-undo-live.png)
- [完成版のRedo](evidence/ime-shared-redo-live.png)
- [完成版の全画面開始・終了10回後](evidence/presentation-shared-10-cycles-live.png)

### 配置比較

- [小さい編集表示、比率 1:1](evidence/layout-small-1-1-live.png)
- [小さい編集表示、比率 1:2](evidence/layout-small-1-2-live.png)
- [大きい編集表示、比率 1:1](evidence/layout-large-1-1-live.png)
- [大きい編集表示、比率 1:2](evidence/layout-large-1-2-live.png)
- [全画面、比率 1:1](evidence/layout-fullscreen-1-1-live.png)
- [全画面、比率 1:2](evidence/layout-fullscreen-1-2-live.png)
- [AeroSpace 無効時の配置座標](evidence/layout-coordinate-log-aerospace-disabled.csv)
- [画像枠とコード枠の描画誤差](evidence/layout-raster-measurements.csv)
- [文字要素の先頭画素の追従測定](evidence/layout-text-anchor-consistency.csv)
- [AeroSpace 有効時の寸法履歴](evidence/layout-window-dimensions-aerospace-enabled.csv)
- [CoreText のフォント解決結果](evidence/font-fallback-coretext.txt)

### IME と入力復旧

- [IME 未確定文字列](evidence/ime-marked-composition.png)
- [候補一覧の AX 取得](evidence/ime-candidate-accessibility.txt)
- [候補ポップアップと未確定文字列](evidence/ime-candidate-region-live.png)
- [候補確定後](evidence/ime-commit-live.png)
- [変換単位の Undo 後](evidence/ime-undo-live.png)
- [変換単位の Redo 後](evidence/ime-redo-live.png)
- [候補範囲の座標ログ](evidence/ime-candidate-coordinate-log.csv)
- [修正版の未確定文字列](evidence/ime-fixed-marked-live.png)
- [修正版の候補ポップアップ](evidence/ime-fixed-candidate-region-live.png)
- [修正版の候補範囲ログ](evidence/ime-fixed-candidate-coordinate-log.csv)
- [タイトル先頭の候補位置](evidence/ime-candidate-position-start-live.png)
- [後続文字列の候補位置](evidence/ime-candidate-position-shifted-live.png)
- [2 か所の候補位置測定値](evidence/ime-candidate-position-measurements.csv)
- [2 か所の入力範囲座標ログ](evidence/ime-candidate-position-coordinate-log.csv)
- [修正版の確定後](evidence/ime-fixed-commit-live.png)
- [修正版の Undo 後](evidence/ime-fixed-undo-live.png)
- [修正版の Redo 後](evidence/ime-fixed-redo-live.png)
- [修正版の再変換候補](evidence/ime-fixed-reconversion-live.png)
- [未確定中の保存停止](evidence/ime-fixed-save-blocked-live.png)
- [未確定中の発表停止](evidence/ime-fixed-presentation-blocked-live.png)
- [Escape 2 回後の空タイトル](evidence/ime-fixed-escape-live.png)
- [確定後の保存復帰](evidence/ime-fixed-save-recovered-live.png)
- [確定後の発表復帰](evidence/ime-fixed-presentation-recovered-live.png)
- [再現条件未特定の二重表示](evidence/ime-fixed-duplicate-observation.png)
- [同じキー列を 10 回保存した値](evidence/ime-repeat-10-saved-values.csv)
- [10 回目の保存後の画面](evidence/ime-repeat-10-save-live.png)
- [修正版の IME 入力録画](evidence/ime-fixed-region-live.mov)
- [文字列だけ確定した場合に残る古い候補](evidence/ime-direct-commit-stale-candidates.png)
- [文字列だけ確定した場合に残る変換状態](evidence/ime-direct-commit-stale-context.png)
- [ネイティブ入力コンテキストを終了して保存](evidence/ime-native-save-commit-live.png)
- [ネイティブ入力コンテキストを終了して発表](evidence/ime-native-presentation-commit-live.png)
- [未確定中の取消後に新しく「桜」を入力](evidence/ime-native-cancel-no-stale-context.png)
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
最終判定には、今回取得した `evidence/shared-layout/` の画像と座標ログを使った。

## 検証コマンド

```sh
~/.cargo/bin/cargo fmt --manifest-path validation/macos-gpui-probe/Cargo.toml --check
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ ~/.cargo/bin/cargo check --manifest-path validation/macos-gpui-probe/Cargo.toml
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ ~/.cargo/bin/cargo test --manifest-path validation/macos-gpui-probe/Cargo.toml
```

3コマンドは成功し、テストは3件すべて合格した。独立グリフ測定は1,473件すべて合格した。
Cargo は依存クレート `block 0.1.6` の将来互換性警告を出した。
