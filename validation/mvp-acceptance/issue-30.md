# Issue #30の製品受け入れ確認

[Issue #30](https://github.com/daaa1k/psycho/issues/30)の検証記録。[保証とテストの対応表](coverage.md)には、親仕様の各保証、Schema全ノード、原文保持試作の10例・26操作を記載した。過去の[README](README.md)の結果と今回の実行を区別する。

原文処理の修正は`5896122`、ネイティブメニューと再変換の修正は`8dc2ce3`。実機検証の実行器は`b8531cb`、`ce491a9`、`035eb00`。原文比較の失敗を先に`40d4f07`で記録した後、引用外の相対字下げ、周囲のタブ、KDLの全8種類の改行境界を修正した。引用内の字下げ・改行・エスケープは変更しない。移動先の改行様式にも引用内改行を採用しない。公開APIは変更していない。

追加の実機検証で[編集メニューの不具合 #44](https://github.com/daaa1k/psycho/issues/44)も再現した。先頭にアプリメニューを設けて「編集」を分離し、編集メニューの自動有効化を止める。これにより製品が計算した空履歴・選択状態の無効指定を維持する。メニュー操作は所有するプロセスの公開Accessibility APIから実際の項目を押し、ショートカットで代用しない。

標準IMEの再変換後にUndoすると本文が空になる[不具合 #45](https://github.com/daaa1k/psycho/issues/45)を再現した。ネイティブコールバックの記録では、空の変換文字列が送られた後も変換が続く。表示中の変換範囲の有無を履歴開始条件に使うと、変換前の全文が空で上書きされる。履歴の開始値を変換の確定まで維持する修正を行った。

## 判定環境

MacBook Air（Mac15,12）、Apple M3、16GB、macOS 26.5.2（25F84）、Xcode 26.5（17F42）、Rust 1.98.1。GPUIはZed `14dd03e89676e7fe74bc205001bfb32c8cfc3952`。仕様の初回判定条件に従い、単一画面1920×1200、画面倍率1で検証する。配置比較の編集ウィンドウは1280×892と1440×992、発表は1920×1200。標準日本語IMEは`com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese`。検証後に入力方式の選択と有効状態を復元する。

Python 3.9、NumPy 2.0.2、Pillow 11.3.0を使用する。フォントの指定は見出し・本文・Captionが`.SystemUIFont`、コードが`Menlo`。実際にGPUIが解決したPostScript名を製品の文字組みから取得した。計測後は依存ソースを全バイト復元し、製品を通常ビルドへ戻した。[実測値](evidence/issue-30/fonts/font-resolution.json)と[復元ビルド記録](evidence/issue-30/fonts/build-restored.txt)を保存した。

| 用途 | 実際のPostScript名 |
| --- | --- |
| 見出し | `.HiraKakuInterface-W6`、`.SFNS-Bold` |
| 本文・Caption | `.HiraKakuInterface-W4`、`.SFNS-Regular` |
| コード | `HiraginoSans-W3`、`Menlo-Regular` |

## 検査方法

`8dc2ce3`の自動テストは[GUIを有効にしない実行](evidence/issue-30/history/tests-public-native-final.log)で109件、[GUIを有効にした実行](evidence/issue-30/history/tests-gui-native-final.log)で112件が成功した。この数には既存の内部境界テストも含む。新規の原文回帰27件と改行・移動13件は、公開操作、実ファイル、独立した期待原文、保存、再読込、Undo/Redoを検査する。内部候補を意図的に壊す試作1操作には製品の公開操作がないため、既存の内部テストとの対応を明記した。

実機ケースは`run_acceptance.py`で順に実行する。ケースごとの期待は各スクリプトのassert、観測結果はログとJSON・CSVに記録する。manifestには開始・終了時刻、リビジョン、作業ツリーの変更有無、画面環境、製品・ソース・実行器のSHA-256、終了コードと合否を記録する。各ケースの前後でハッシュを照合し、途中でファイルが変わった実行は合格に数えない。

配置比較では全256グリフの識別子・改行・基準座標を照合する。描画は同率拡縮した画像の固定領域を、強い画素と弱い画素で物理1ピクセル以内に両方向で照合する。アンチエイリアスの濃淡差だけを許容し、部品ごとの位置補正はしない。直接編集20ケースでは実際のカーソルとInspector表示も検査する。画像は撮影した物理解像度で保持する。

録画は検証で起動した製品の単一ウィンドウだけをScreenCaptureKitで撮影する。H.264、1920×1080、音声なしで、終了時にファイルを正常に確定する。録画は操作の確認用であり、物理画素の比較には元のPNGと座標CSVを使う。別ウィンドウのIME候補はネイティブ画面領域の撮影と候補ウィンドウ座標で確認する。

## 結果

必須の単一画面環境で30ケースが成功した。製品バイナリー・全製品ソース・OS・画面環境の同一性を照合し、5実行から16＋5＋6＋2＋1ケースを採用した。単一実行で30件が成功したという判定ではない。最終の独立受け入れレビューでも、必須ケースの不足と過大な合格主張がないことを確認した。

| ケース | 期待 | 観測 | 合否 | 証跡 |
| --- | --- | --- | --- | --- |
| `sample` | 6枚で本文・コード・構造・列幅・画像を編集し、Undo/Redo、明示保存、再読込、発表の前後移動と終了を確認する | 8種の編集で保存原文とUndo/Redoが一致し、6枚を再読込して発表の前後移動・終了が成功 | PASS | [ログ](evidence/issue-30/final/sample.log) |
| `blank` | 白紙Slideへ6種類のElementを追加し、Slide移動・Undo/Redo・保存を確認する | 6種類のElement追加とSlideの追加・移動・削除・復元が成功し、Image削除後も実画像を保持 | PASS | [ログ](evidence/issue-30/final/blank.log) |
| `code-colors` | Rust・YAML・JSON・TOMLの着色と、未対応言語・言語なしの表示を確認する | Rust・YAML・JSON・TOMLは着色され、未対応言語・言語なしは無着色 | PASS | [ログ](evidence/issue-30/final/code-colors.log) |
| `column-width-drag` | 連続プレビュー、下限1・上限99・合計100、ドラッグ一回のUndo/Redo、保存再読込を確認する | 幅65と下限1・上限99を観測。保持中の保存は原文不変、完了後は一回Undo/Redoで全文一致 | PASS | [ログ](evidence/issue-30/final/column-width-drag.log) |
| `edit-menu` | ネイティブ編集メニューの操作名・空履歴無効表示・入力と文書のUndo/Redo・Copy/Cut/Pasteを確認する | 空履歴が無効、操作名が更新。native項目のCopy/Cut/Pasteと入力・文書のUndo/Redoが成功 | PASS | [ログ](evidence/issue-30/final/edit-menu.log) |
| `ime-reconversion` | 標準IMEで再変換・候補変更・確定し、全文の一回Undo/Redo、保存再読込を確認する | 標準IME候補を変更して確定し、全文が一回Undo/Redoで復元。保存・再読込が一致 | PASS | [ログ](evidence/issue-30/final/ime-reconversion.log) |
| `layout` | 編集2サイズと発表で全グリフ・折返し・基準座標を照合し、描画像を物理1px以内で比較する | 6画面それぞれ全256グリフが一致。編集2サイズと発表の描画像比較が物理1px以内 | PASS | [ログ](evidence/issue-30/final/layout.log) |
| `direct-layout` | 直接編集20ケースでグリフ位置、折返し、1px描画、実カーソルとInspector表示を確認する | 直接編集20ケースでグリフ・折返し・描画像比較と実カーソル・Inspectorの検査が成功 | PASS | [ログ](evidence/issue-30/final/direct-layout.log) |
| `ime` | 標準日本語IMEの候補・確定・移動・履歴・保存・取消・無効入力を確認する | 20状態のnative変換・候補位置・キー優先・履歴・取消・無効入力の検査が成功 | PASS | [ログ](evidence/issue-30/final/ime.log) |
| `ime-caption` | CaptionのCanvasとInspectorの往復で変換状態と候補位置を確認する | 5種のnative変換状態とCaptionのCanvas／Inspector往復、別欄への確定が成功 | PASS | [ログ](evidence/issue-30/final/ime-caption.log) |
| `presentation-ime` | 変換中の発表開始で確定し、終了後の編集・保存・Undoを確認する | 未保存の変換を確定して発表し、終了後のCanvas・明示保存・Undoが成功 | PASS | [ログ](evidence/issue-30/final/presentation-ime.log) |
| `keys` | 全画面開始と終了を10回反復し、キー・ポインター・Finder復帰・既存全画面からの発表を確認する | 10回の全画面往復、全キー・ポインター、Finder復帰、既存全画面からの開始が成功 | PASS | [ログ](evidence/issue-30/final/keys.log) |
| `presentation` | 開始可否、固定snapshot、開始位置、保存の扱い、外部変更後の復帰を確認する | 開始禁止・固定snapshot・開始位置・外部変更時の凍結と復帰が成功 | PASS | [ログ](evidence/issue-30/final/presentation.log) |
| `text` | 直接編集、選択・削除、コードのTab、言語変更、入力履歴を確認する | 直接編集・コードTab・言語変更・入力履歴・明示保存・全文Undo/Redo・再読込が成功 | PASS | [ログ](evidence/issue-30/final/text.log) |
| `no-edit-save` | 通常・生・複数行引用とCRLFの無編集保存で原文全バイトを保持する | 通常・生・複数行引用とCRLFで保存バイト不変、空履歴を保持 | PASS | [ログ](evidence/issue-30/final/no-edit-save.log) |
| `redo` | 欄内Undo後の編集終了で文書履歴へ引き継ぎ、Redoを保持する | Escapeで編集を終えた後も文書履歴のRedoが残り、保存原文が一致 | PASS | [ログ](evidence/issue-30/final/redo.log) |
| `columns` | 列内外・別Columns・各挿入位置の移動、数値入力と無効値、履歴と保存を確認する | 列内外・別Columns・挿入位置の移動と全文Undo/Redo、再起動後の表示、無効幅の保存禁止が成功 | PASS | [ログ](evidence/issue-30/final-tail/columns.log) |
| `layout-repair` | 画面外と非表示Slideの診断、全文修正、一覧のページ切替、保存と再読込を確認する | 非表示Slide・画面外の診断、Inspector修正、一覧ページ切替、保存・再読込が成功 | PASS | [ログ](evidence/issue-30/final-tail/layout-repair.log) |
| `following-layout` | 空Caption・縦長画像・列内要素の後続配置を編集と発表で確認する | 空Caption・縦長画像・列内後続要素の実測座標が編集と発表で期待範囲に一致 | PASS | [ログ](evidence/issue-30/final-tail/following-layout.log) |
| `save-as-retry` | 別名保存の失敗で原本・入力・履歴を保持し、権限復旧後の同じ保存先へ再試行する | native別名保存の書込み失敗後も原文と入力を保持し、同じ保存先への再試行と履歴初期化が成功 | PASS | [ログ](evidence/issue-30/final-tail/save-as-retry.log) |
| `cursor` | 発表中のnativeカーソルを無操作時に隠し、移動と終了で再表示し、クリック・スクロールでSlideを変えない | native APIで初期表示・2秒無操作後の非表示・移動後と終了後の表示を確認し、クリック・スクロールでSlide不変 | PASS | [ログ](evidence/issue-30/final-tail/cursor.log) |
| `close` | 未保存時の保存・破棄・取消と、発表カーソルの非表示・再表示を確認する | 取消後は入力・原本保持、保存／破棄後は対象ウィンドウが消滅し、保存先の全バイトが期待値に一致 | PASS | [ログ](evidence/issue-30/final-rest/close.log) |
| `input-scope` | 入力内容を欄別に扱い、編集終了時に文書の履歴へ引き継ぐ | 入力欄のtoolbar Undo/Redo境界と、保存後の文書履歴の検査が成功 | PASS | [ログ](evidence/issue-30/final-rest/input-scope.log) |
| `history-limit` | 入力履歴の1000操作上限と、文書履歴への引き継ぎを確認する | 1,001入力で履歴1,000件、local Undoの基準値保持、文書の全Redo・全Undoと保存原文が一致 | PASS | [ログ](evidence/issue-30/final-rest/history-limit.log) |
| `bullet-items` | 箇条書きの空項目・追加削除・並替え・入力履歴を確認する | 空項目からの追加・入力・並替え・削除と、各操作の一回Undo/Redoで保存原文が一致 | PASS | [ログ](evidence/issue-30/final-rest/bullet-items.log) |
| `nested-bullet-items` | 列内の箇条書き項目を編集し、履歴と保存を確認する | 列内で同じ項目操作と原文履歴が一致し、隣の本文を保持 | PASS | [ログ](evidence/issue-30/final-rest/nested-bullet-items.log) |
| `assets` | 実画像のEXIF・透過・形式の画素と、別名保存後の参照・原本保持を確認する | JPEG・PNG・WebPの画素検査が一致し、別名保存後の参照、再保存と原本全バイトを保持 | PASS | [ログ](evidence/issue-30/final-rest/assets.log) |
| `save-failure` | 書込み失敗で原本・入力・履歴を保持し、再試行とUndoで保存バイトを復元する | 書込み失敗後は原文と編集値を保持し、再保存で期待原文と一致。一回Undoで原本全バイトへ復元 | PASS | [ログ](evidence/issue-30/final-complete/save-failure.log) |
| `conflict` | 外部変更後の凍結で入力・適用・保存・Undoを無効にし、未確定の入力を保持してSlide閲覧を許可する | 各拒否操作後もnative入力と外部原文が不変。Slide 2を閲覧しても未確定の入力を保持 | PASS | [ログ](evidence/issue-30/final-complete/conflict.log) |
| `diagnostics` | 構文・Schema診断から読み取り専用のソース行へ移動し、画像診断から対象要素へ移動する | 構文3:15とSchema4:3／5:3を選択し、同じソース行の描画と入力後の原文不変を確認。画像診断はSlide 2の画像を選択し、Undoで選択・原文を維持 | PASS | [ログ](evidence/issue-30/final-diagnostics-4/diagnostics.log) |

[採用した30ケース](evidence/issue-30/accepted-cases.json)は元manifest・ケースログ・ログのSHA-256を一意に参照する。採用する際に、不足・重複・終了コード非zero・製品やソースの不一致を拒否した。全製品ソースは`8dc2ce3`のgit内容とも一致する。manifestの`dirty`は検証器・文書・証跡の作成中の状態を記録しており、製品ソースの変更を隠すものではない。

[初回実行](evidence/issue-30/final/manifest.json)は16ケースが成功し、Columnsの読み取りで`app_not_found`となって実行全体はFAILだった。[次の実行](evidence/issue-30/final-tail/manifest.json)は5ケースが成功し、破棄ボタンのOCR認識でFAILとなった。[3番目の実行](evidence/issue-30/final-rest/manifest.json)は6ケースが成功し、保存失敗検証の古い固定座標で入力できずFAILとなった。[入力欄のOCR差異を記録した実行](evidence/issue-30/final-last/manifest.json)も未合格として保存した。元のFAILは合格へ書き換えていない。[最後の実行](evidence/issue-30/final-complete/manifest.json)では保存復旧と競合の2ケースが成功した。

Columnsの観測失敗は、[OSログ](evidence/issue-30/history/owned-observation-system.log)ではproviderの照会後にプロセス終了が記録され、検証器の終了処理と時刻が一致した。失敗の原因は未確定であり、製品クラッシュや原因修正済みと断定しない。同一製品の診断用Columns再実行は全操作が成功し、製品stderrは空だった。採用した実行では、所有PIDの生存と新しいアプリ一覧への登録を確認した場合だけ、読み取り観測を1回再試行する。入力・クリック・履歴操作は再送しない。

診断選択の不足を独立監査で発見し、[追加実行](evidence/issue-30/final-diagnostics-4/manifest.json)で検証した。構文3:15とSchema4:3・5:3のファイル・行・列を実画面で確認し、診断行と選択行の描画を双方向で物理1px以内に照合した。入力・Backspace・保存後も表示と原文全バイトを保持した。画像診断からSlide 2の画像へ移動し、選択だけで履歴を作らないことも確認した。小さい文字のOCR誤読による[初回](evidence/issue-30/final-diagnostics/manifest.json)・[2回目](evidence/issue-30/final-diagnostics-2/manifest.json)・[3回目](evidence/issue-30/final-diagnostics-3/manifest.json)のFAILも保存した。[録画の全フレーム検査](evidence/issue-30/final-diagnostics-4/diagnostics/media-verification.json)と元PNGを残した。

検証器の差分は、所有プロセスのstderr記録と読み取り回復、観測済みOCRの別名、保存復旧・競合の共有GUI検証器への移行と診断選択の追加に限定した。移行後は実際の入力欄のfocusと値を確認し、原文の専用コピーだけを書込み禁止にする。ウィンドウID消滅・原文全バイト一致・native入力状態の検査は維持した。検証器が同一であるとは主張せず、各実行の全検証コードSHA-256と変更範囲を保存した。

全画面10回の[操作録画](evidence/issue-30/final/keys/keys/window-23135.mov)と[録画のデコード確認](evidence/issue-30/final/keys/media-verification.json)、IME再変換の[操作録画](evidence/issue-30/final/ime-reconversion/window-9192.mov)と[録画のデコード確認](evidence/issue-30/final/ime-reconversion/media-verification.json)を記録した。各録画はH.264、1920×1080、音声なしで、全フレームのデコードを確認した。

物理画素の比較に使った代表画像は[小さい編集ウィンドウ](evidence/issue-30/final/layout/layout-small-1.png)、[大きい編集ウィンドウ](evidence/issue-30/final/layout/layout-large-1.png)、[発表ウィンドウ](evidence/issue-30/final/layout/layout-fullscreen-1.png)である。録画の1920×1080への変換画像を物理画素比較には使わない。

最初の補助測定は内蔵Retinaと外部ディスプレイの2画面接続だった。基準座標は全256グリフで一致したが、小さい編集ウィンドウの本文の画素比較が80／5画素で不一致だった。この結果は未合格であり、必須の単一画面環境の結果へ混ぜない。複数画面・Retina環境全般の合格は主張しない。[Issue #46](https://github.com/daaa1k/psycho/issues/46)で原因調査を追跡し、[補助測定の失敗記録](evidence/issue-30/supplemental-retina/layout.log)を保存した。

先行検証で一度観測したIMEの二重表示は[Issue #31](https://github.com/daaa1k/psycho/issues/31)で継続追跡する。キー列が残っておらず、利用者の並行操作の可能性もあるため原因は未確定である。今回の検証結果を、この過去の観測の原因修正とは呼ばない。Issue #31は未修正として追跡を続ける。

親仕様[Issue #17](https://github.com/daaa1k/psycho/issues/17)は変更せず、閉じない。

## 再実行

OrcaとSwiftにアクセシビリティ・画面記録権限がある、画面ロックされていない単一画面のMacで実行する。GUI操作中はキーボードとポインターを操作しない。検証は所有するプロセスと`target/`以下のコピーを使う。出力先は未作成のディレクトリーを指定する。

```sh
python3 -m venv target/mvp-acceptance-python
target/mvp-acceptance-python/bin/pip install -r validation/mvp-acceptance/requirements.txt
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang \
  CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo test --locked --features gui
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang \
  CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo build --locked --features gui
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang \
  CC=/usr/bin/clang CXX=/usr/bin/clang++ \
  target/mvp-acceptance-python/bin/python validation/mvp-acceptance/font_resolution_acceptance.py \
  --output target/mvp-font-resolution-new
target/mvp-acceptance-python/bin/python validation/mvp-acceptance/run_acceptance.py \
  --output target/mvp-acceptance-new
```

必要なケースだけを実行する場合は`--case sample --case ime`のように指定する。失敗時は最初の失敗で停止する。出力先を再利用して証跡を上書きしない。画像と保存失敗の既存検証は専用の`target/`コピーを使い、変更されたコピーを黙って置き換えない。
