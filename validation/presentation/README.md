# Issue #29 の全画面発表

[Issue #29](https://github.com/daaa1k/psycho/issues/29)の公開操作とmacOS実機を検証する。最終製品ソースは`c06369f`。モデル、読み込み済み画像、発表中の位置を`PresentationSession`にまとめ、入力確定後の外部同期を一度だけ実行する。配置判定と描画には編集画面と同じGPUIの処理を使う。

## 環境

Apple M3、macOS 26.5.2（25F84）、arm64。GPUIはZed `14dd03e89676e7fe74bc205001bfb32c8cfc3952`。編集ウィンドウは1280×892、配置比較の大きい編集ウィンドウは1440×992、全画面は1920×1200。撮影倍率は1。本文は`.SystemUIFont`、コードはMenlo。すべて検証専用のPIDと`target/`内のファイルで操作する。

## 実行方法

新しい出力ディレクトリーを指定する。同名の既存検証ファイルは上書きしない。GUI操作は順番に実行する。

```sh
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang \
  CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo test --locked --features gui
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang \
  CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo build --locked --features gui
python3 -m venv target/presentation-python
target/presentation-python/bin/pip install Pillow numpy
target/presentation-python/bin/python validation/mvp-acceptance/presentation_workflow_acceptance.py \
  --output target/presentation-workflow
python3 validation/mvp-acceptance/close_cursor_acceptance.py \
  --cursor-only --output target/presentation-cursor
target/presentation-python/bin/python validation/mvp-acceptance/presentation_layout_acceptance.py \
  --output target/presentation-layout
xcrun swiftc validation/mvp-acceptance/with_japanese_ime.swift -o target/with-japanese-ime
target/with-japanese-ime /usr/bin/python3 validation/mvp-acceptance/presentation_ime_acceptance.py \
  --output target/presentation-ime
```

IME用ラッパーは標準日本語入力を選び、子プロセスの終了後に選択と有効化状態を戻す。SIGINTとSIGTERMでは子プロセスを終了してから設定を戻す。SIGKILLでは終了処理を実行できない。

`presentation_workflow_acceptance.py`は`--section snapshot`、`preflight`、`keys`、`positions`、`external`で対象を限定できる。複数指定もできる。既定では全対象を実行し、開始・終了を10回反復する。`--cycles`は反復数を変更する。

## 結果

`cargo test --locked --features gui`は72件成功した。[テスト結果](evidence/tests.log)には開始拒否、未保存内容と画像の固定、移動の境界、再読込後の選択復帰を含む公開操作のテスト7件も記録した。

| 項目 | 実機で確認した結果 | 証跡 |
| --- | --- | --- |
| 開始前の検証 | 0枚、無効入力・文書、現在以外のSlideのはみ出し、開始直前に削除した画像で編集画面に留まる | [開始拒否](evidence/preflight.json) |
| 内容と画像の固定 | 未保存の見出しを表示して元ファイルを保持。画像を更新・削除しても赤のまま、終了後の再開始では青を表示 | [観測](evidence/snapshot.json)、[固定した画像](evidence/frozen-image.png)、[再読込した画像](evidence/reloaded-image.png) |
| 外部変更後の選択 | 発表中は旧内容を表示し、終了時に同じid、同じ順番、末尾、未選択の順で復帰 | [選択復帰](evidence/positions.json) |
| 競合・削除・無効文書 | 発表中の表示を維持。終了後は開始を拒否。競合では別名保存で元ファイルを保持し、退避した内容を発表 | [外部変更と退避](evidence/external.json) |
| 共通配置 | 2枚それぞれの256文字について、編集画面2サイズと全画面で文字・基準位置・改行が一致。文字の濃い部分の差は1物理ピクセル以内 | [配置結果](evidence/layout.log)、[文字座標](evidence/layout-glyph-comparison.csv)、[画像比較](evidence/layout-raster-comparison.csv) |
| キーと復帰 | 全7移動キーと先頭末尾の境界、クリック・スクロールで位置を維持。他アプリから戻り、10回の開始・終了後も正常。開始前の全画面状態を維持して復帰 | [キーとウィンドウ](evidence/keys.json)、[各キーによる移動](evidence/key-movement.json) |
| カーソル | 静止2秒後に非表示、移動で再表示、終了後も表示。`CGCursorIsVisible`で観測 | [カーソル](evidence/cursor.json) |
| 日本語IME | 未確定文字を開始時に確定。同じ文字列を発表し、終了後は文字編集を再開せず、明示保存で初めてファイルを更新 | [IME観測](evidence/ime.json)、[未確定入力](evidence/ime-marked.png)、[発表画面](evidence/ime-presentation.png)、[キャンバス復帰](evidence/ime-canvas-return.png) |
| 入力環境の復元 | SIGINTとSIGTERMのどちらでも、選択中の入力ソースと有効化状態を実行前へ戻す | [復元結果](evidence/ime-restoration.log) |

開始拒否、画像固定、選択復帰、外部変更は`b83d4df`の実機検証から該当項目だけを抽出した。検証中に、理由文言の誤った期待値やmacOSのウィンドウメニューによる操作の遮断を修正したため、各記録に元の実行ディレクトリーを残している。キーと復帰、カーソル、配置、IMEは最終ソース`c06369f`で確認した。`b83d4df`との差分は、発表前から全画面だった編集画面へ全画面のまま戻る処理だけである。

IMEの証跡は開始前の未確定状態、発表画面、終了後のキャンバスと入力状態を記録した。ネイティブ録画は黒い映像や終了待ちの失敗があったため、検証結果には採用していない。

Slideのみの表示と16:9の黒帯は画面で確認した。切替アニメーションは、描画が現在位置を直接参照し、移動処理で位置の更新と再描画だけを実行することをソースで確認した。フレーム時間の測定は行っていない。
