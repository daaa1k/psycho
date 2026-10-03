# Issue #17 製品受け入れ確認

2026-10-03に製品バイナリーで実施した確認を記録する。配置比較と全画面10回は`0ec3116`。保存失敗の再試行は`f66d593`、競合中の入力保持と再読み込みは`6ccb562`で確認した。自動テスト39件は`6ccb562`で再実行した。先行試作の結果は製品の合格証拠として数えない。Issue #17全体の判定は**未完了**。下表の確認済みケースと、末尾の残件を分けて扱う。

## 環境

MacBook Air / Apple M3、macOS 26.5.2（25F84）、Xcode 26.5、Rust 1.98.1。GPUIとGPUI platformはZed `14dd03e89676e7fe74bc205001bfb32c8cfc3952`。内蔵画面1枚、Orca 1.4.218。GPUIの`window.scale_factor()`と撮影倍率は1。編集ウィンドウは1280×892と1440×992、viewportはそれぞれ1280×860と1440×960。全画面は1920×1200。スライドの表示倍率は0.646875、0.771875、1.5。

本文は`.SystemUIFont`、コードはMenloを指定した。日本語の代替フォントはOSのCoreTextに任せる。先行検証の同一OS・GPUI環境ではMenloの日本語をHiraginoSans-W3、絵文字をAppleColorEmojiに解決している。今回は代替フォント名をGPUIから直接取得していない。配置比較では実際に選ばれたグリフIDが3サイズで一致することを検査した。

IME確認時はmacOS標準日本語入力 `com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese` を使用した。確認後は元のazooKeyに戻し、この確認のために一時的に有効化したKotoeriを無効化した。

## 結果

| ケース | 期待結果 | 観測結果 | 判定 |
| --- | --- | --- | --- |
| 公開操作の自動テスト | 原文保持、Schema、編集、履歴、保存失敗・競合・退避、画像診断が公開操作から確認できる | [cargo-test.txt](evidence/cargo-test.txt)。各テスト内の組合せを含む | 合格 |
| IME入力・再変換 | 「日本語変換」の入力と再変換で欠落・重複がなく、確定を1操作でUndo/Redoできる | 標準IMEでキー入力、Spaceによる候補、Ctrl+Shift+Rによる再変換、Return、Undo/Redoを確認。保存値も一致 | 合格 |
| IME中の保存・発表 | 未確定入力を反映する。発表開始は自動保存しない | 保存ボタンで「保存」を確定・保存。発表ボタンで「発表」を確定・表示し、ディスクは「保存」のまま。終了後の保存で「発表」に更新 | 合格 |
| 無効入力 | 空タイトルは保存・発表を止め、理由を欄に示す。明示取消で復帰する | 空タイトルで両操作を停止。欄の日本語エラー、未保存表示、取消後の有効値復元を確認 | 合格 |
| 列をまたぐドラッグ | 挿入先・候補列を表示し、ドロップ後に表示を消す。移動はUndo 1操作 | 左の箇条書きを右のコード前へ移動。挿入線・列背景・ゴーストを撮影。保存後、Undo 1回と保存で移動前ファイルに全バイト一致 | 合格 |
| 列幅ドラッグ | 幅を連続更新し、終了までをUndo 1操作にする。はみ出しは保存可・発表不可 | 45:55から56:44へ変更。コードのはみ出し診断と発表停止、保存を確認。Undo 1回と保存で変更前ファイルに全バイト一致 | 合格 |
| 共通配置 | 日本語、英数字、長いURL、箇条書き、タブ入りコード、複数行Caption、50:50と33:67の列で改行・1280×720基準座標が一致する | 2枚×3サイズで各238グリフを照合。基準座標誤差0.001未満、描画丸め1物理px以内。強い描画画素は両方向で相手の弱い画素から1px以内 | 合格 |
| 全画面反復 | 両開始位置、全7移動キー、先頭末尾、Escape、終了後の編集フォーカスを10回確認する | [fullscreen-checks.csv](evidence/fullscreen-checks.csv)の160状態を見出しOCRとウィンドウ寸法で検査。10回とも合格。終了後のSpaceで発表が再開しない | 合格 |
| 別アプリ往復 | 発表から別アプリへ切替え、戻った後もキー移動・終了できる | Finderへ切替え、psychoへ復帰、Right/PageDown/Left/Spaceによる移動とEscapeを確認 | 合格 |
| 発表中の画像削除 | 開始後は読み込んだ画像を表示し続け、再開始時には現在の欠落を検出する | 発表中にscratch PNGを移動し、画像表示が不変。終了後に欠落診断とCaptionを表示、再開始を停止。画像を戻して再読込・保存で復帰 | 合格 |
| 外部の正常変更・無効化・削除 | 正常更新は選択位置を保って読む。無効化・削除では最後の正常表示と理由を残し、復旧後に読み直せる | Slide 3を保って正常更新を反映。構文エラーと削除で停止し、復旧後の再読み込みで編集を再開 | 合格 |
| 外部競合・未確定入力 | 競合中は入力・保存・履歴を止め、保持中の入力を失わずにプレビューを移動できる | 入力変更、適用、Cmd+S、Cmd+Z、Slide選択を試し、保持値と元ファイルが変わらないことをOCR・実ファイルで確認。修正前に見つけたInspectorからの編集継続を修正 | 合格 |
| 再読み込みの確認 | 未保存内容の破棄を確認し、取消時には保持、続行時には破棄して再開する | 取消と続行を別々に確認。続行後は保持中の入力が保存済み値に戻り、未保存表示と履歴が消える | 合格 |
| GUIからの退避 | 外部ファイルを変えずに別フォルダーへ退避し、既存先は上書き確認する | Native Save panelで既存先の取消とReplaceを確認。未保存内容を保存し、外部ファイルを保持。退避後の画像表示も確認 | 合格 |
| 書込み失敗からの再試行 | 原本・編集中内容・履歴を保ち、権限を戻したら同じ保存操作で再試行できる | 一時ファイル作成の権限エラーを外部読込エラーとして扱う不具合を修正。失敗中の原本不変、再試行成功、Undo 1回と保存で全バイト一致を確認 | 合格 |

[IME録画](evidence/ime-standard.mov)、[全画面キー操作の録画](evidence/fullscreen.mov)を保存した。録画は確認の一部を示し、全ケースの連続録画ではない。全画面10回の各開始・終了画像と1回目の全キー画像を残した。画像削除、ドラッグ、取消などの静止画は`evidence`内にある。文字描画修正前のIME・ドラッグ画像はその機能の観測用であり、最終配置の証拠には使わない。最終配置は`layout-*.png`と`glyph-origins.csv`を使う。

座標比較は提出したグリフ座標を共通のスライド原点から1280×720へ戻し、全画面の同じグリフと照合する。描画比較は全画面画像を同率で縮小し、固定した文字領域で強い画素と弱い画素を1pxの範囲で両方向に照合する。部品ごとの位置補正や、画像ごとの閾値変更は行わない。独立したCoreText参照の再生成は今回の製品比較には含めていない。

## 再実行

作業中の文書を閉じ、検証用のコピーを開く。GUI操作中はキーボード・ポインターを操作しない。OrcaとSwiftに必要なアクセシビリティ・画面記録権限がある環境で実行する。

```sh
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ \
  ~/.cargo/bin/cargo test --features gui
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ \
  ~/.cargo/bin/cargo build --features gui --bin psycho
PSYCHO_GLYPH_TRACE=target/mvp-glyphs.csv ./target/debug/psycho validation/mvp-acceptance/layout.kdl
# 別のターミナルで実行。出力先を指定しなければtarget/mvp-acceptanceに保存する。
python3 validation/mvp-acceptance/capture_layout.py
python3 -m venv target/mvp-layout-venv
target/mvp-layout-venv/bin/pip install numpy==2.0.2 Pillow==11.3.0
target/mvp-layout-venv/bin/python validation/mvp-acceptance/measure_layout.py --evidence target/mvp-acceptance
```

測定だけなら最後のコマンドの`--evidence`を`validation/mvp-acceptance/evidence`へ変更する。全画面反復は6枚の`examples/build-time.kdl`のコピーを開き、初期1280×892の編集ウィンドウで実行する。

```sh
python3 validation/mvp-acceptance/fullscreen_acceptance.py --record
```

書込み失敗と競合の回帰確認は、次のコードで専用のscratch文書を作成して実行できる。`--prepare`が表示した起動コマンドでpsychoを開き、その後に同じスクリプトを`--prepare`なしで実行する。プロセス引数が専用文書を指すことを検査してからGUIを操作する。

```sh
python3 validation/mvp-acceptance/save_failure_acceptance.py --prepare
# 表示されたコマンドでpsychoを起動する
python3 validation/mvp-acceptance/save_failure_acceptance.py
# 上の専用文書を閉じてから次へ進む
python3 validation/mvp-acceptance/conflict_acceptance.py --prepare
# 表示されたコマンドでpsychoを起動する
python3 validation/mvp-acceptance/conflict_acceptance.py
```

ドラッグの再実行には`slow_drag.swift PID x1 y1 x2 y2 [hold_seconds]`を使える。座標は最新のpsychoウィンドウ内で指定する。`hold_seconds`を長くするとドロップ前の表示を撮影できる。Orcaの一括dragが短すぎるとGPUIの描画が間に合わなかったため、同じMacのCGEventsを段階的に送る補助コードを残した。PIDがpsychoであり、前面にあることを検査してから送信する。

## 残件

Issue #17全体を合格にするには、次の製品上の確認を続ける必要がある。

- 標準IMEの候補位置をカーソル移動・ウィンドウ移動でも測定し、変換中のEnter/Escape/矢印が編集操作へ二重作用しないことを確認する。
- キャンバスを直接編集中の文字配置・タブ・候補位置を確認する。今回の3サイズ比較は入力フォーカスを外した編集表示を対象とした。
- 6枚の通し操作と白紙Slideから全ElementをGUIで追加する操作を最後まで確認する。複数Columns間・列内外・不正な入れ子のドラッグと中身の削除復元もGUIで確認する。
- 退避失敗からの再試行と、画像パス欄を選択した状態で異なる深さのフォルダーへ退避し、再保存したときの参照をGUIで確認する。通常保存の失敗・再試行、外部変更・競合・削除・無効化・破棄確認・別フォルダーへの退避は確認済み。
- 製品GUIでEXIF回転と透過を確認する。自動テストは受理と診断を検査し、描画方向までは検査していない。
- 空列・高い列の後続配置、画像の全体表示、画面外Elementの全文修正、現在Slide以外の問題による発表停止、発表中の外部変更後の復帰位置を確認する。

保存時の2回の比較とatomic renameを検査したが、同時書込みをrename直前に注入する決定的な試験はまだ追加していない。この残件を含め、未確認事項をIssueの完了扱いにしない。
