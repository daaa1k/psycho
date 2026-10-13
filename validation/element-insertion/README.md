# 挿入操作の受け入れ結果

[Issue #52](https://github.com/daaa1k/psycho/issues/52) の挿入操作を、実際の macOS 製品ウィンドウから検証した。見出し、本文、箇条書き、コード、画像、2列を日本語の名前、説明、SVGで表示する。スライド直下と各2列の左右へ、ツールバー、Inspector、macOSメニューから同じ追加処理を実行する。

## 実行環境

最終の通し検証、VoiceOver、古い要求の実行拒否は `fca3fd4` の製品ソースで実施した。基点は `fd8f29a`。狭い画面、無効入力、競合、0枚、IME、ネイティブ画像の初回証跡は `cb9ad79` で取得した。両版の製品コード差分はラジオ選択値の公開だけであり、文書操作は同じ。macOSは26.5.2、ビルド番号は25F84。接続画面は1920×1200の1枚で、backingScaleFactorは1.0。通常ウィンドウは1280×892、狭いウィンドウは1024×700で確認した。ライト表示は1024×700、1280×892、1440×960で確認した。ウィンドウ寸法はタイトルバーを含む。

この環境ではNixのGCCが `cc` を提供する。GUIのリンクには `PATH="/usr/bin:/bin:$PATH"` でXcodeのClangを使用した。検証用プロセスとファイルは未使用の `target/issue-52-*` 配下に作り、終了時はそのプロセスだけを停止した。

## 確認結果

| 操作 | 結果と記録 |
| --- | --- |
| 白紙への全6種類の追加 | 追加直後の選択と文字編集、画像パスへのフォーカスを確認した。[全操作記録](evidence/full-observations.json)。 |
| 各種類のUndoとRedo | 1回のUndoで元のKDLへ、1回のRedoで追加後のKDLへ戻った。記録内の6件の `added`、`undone`、`redone` は実際の保存値。[実行ログ](evidence/full-run.log)。 |
| 画像削除と履歴 | 削除、Undo、Redo、再UndoでKDLを照合し、画像ファイルのバイトが変わらないことを確認した。[編集画面](evidence/all-kinds.png)。 |
| 複数の2列への追加 | ツールバーから2つの2列の全左右へ追加した。ネイティブメニューから4つの2列の全左右へ追加し、保存KDLの親と左右を照合した。Inspector指定の左列も照合した。[列の記録](evidence/edge-observations.json)。 |
| 直下の既定値と入れ子拒否 | メニューを開き直すと直下が選択された。列内への2列追加は無効になった。[メニュー](evidence/menu.png)。 |
| キーボード操作 | 狭いウィンドウで4つの2列を置き、Tabで画面外の本文項目へ移動するとスクロールが追従した。Returnで直下へ追加し、Escapeで閉じた。[画面](evidence/small-window.png)。 |
| 無効な列幅入力 | ツールバー、Inspector、ネイティブからの追加は文書を変えず、入力値0とフォーカスを保持した。[画面](evidence/invalid-value.png)。 |
| 外部競合 | ツールバーは無効、Inspectorの入口は非表示になった。ネイティブの深いメニューにある9つの本文項目はすべて無効だった。[属性](evidence/conflict-native.json)、[画面](evidence/conflict.png)。 |
| 0枚 | ツールバーの作成導線をReturnで実行し、ネイティブ挿入メニューからも1枚作成した。[記録](evidence/empty-observations.json)。 |
| 古い追加先 | 左列の要求を表示した後、外部で先頭の2列を削除した。同じindexに別の有効な2列が残る状態で、本文の公開AXPressを実行した。拒否メッセージが出てKDLは変わらなかった。[ログ](evidence/stale.log)、[画面](evidence/stale-refused.png)。 |
| 日本語IME | 標準日本語IMEで変換中の「か」を保持して挿入を開き、見出し追加後に本文の「か」を保存した。入力ソースと有効状態を元へ戻した。[記録](evidence/ime-observations.json)、[復元ログ](evidence/ime.log)。 |
| VoiceOver | 「スライド直下 選択中 radio button」、左列の「選択中」、「2列 淡色表示 button」、本文buttonを実際に読み上げた。VoiceOverから左列を選び、本文を追加して保存した。[読み上げ](evidence/voiceover-observations.json)、[設定復元](evidence/voiceover-restoration.json)。 |
| 保存、再読み込み、発表 | 全種類を含む2枚の文書を保存、再起動し、原文の一致を確認した。両スライドを全画面で発表し、終了後は2枚目へ戻った。[KDL](evidence/presentation.kdl)、[発表画面](evidence/present-content.png)、[列の発表](evidence/present-columns.png)。 |
| ライト表示と寸法 | 3つのウィンドウ寸法で6種類の名前とアイコンを確認した。開閉だけではdirtyと履歴が変わらず、本文追加も保存できた。[記録](evidence/light-observations.json)、[最大寸法](evidence/light-1440x960.png)。 |
| ネイティブ表示 | アイコンと説明を実メニューの画像で確認した。撮影対象は検証プロセスが所有するメニューだけ。[画像](evidence/native-menu.png)。 |

VoiceOverのラジオ選択には、GPUIの `aria_selected` に加え、macOSのラジオ値へ変換される `aria_toggled` が必要だった。同じ選択値から両者を作る。[Appleのグループ操作](https://support.apple.com/guide/voiceover/interaction-commands-cpvokys07/mac)に従い、挿入グループへControl-Option-Shift-Downで入った。

IME候補表示中は、指定ウィンドウの画像が製品でもAXツリーが候補ダイアログになる場合があった。このケースは実画像の挿入ボタンをクリックし、入力状態と保存値を照合した。発表中の編集側 `currentSlide` は発表位置を示さないため、実画像の本文で両スライドを照合した。

## 再実行用の検証

`validation/mvp-acceptance/` に次の公開操作スクリプトを追加した。すべて `--output` に未使用の `target/` 配下を必要とする。

- `insertion_acceptance.py` は全6種類、履歴、複数親、画像、保存、再起動、発表を確認する。
- `insertion_edge_acceptance.py` は狭い画面、ネイティブ全左右、Inspector、無効入力、競合を確認する。`--ime` は日本語入力、`--conflict-only` は競合だけを確認する。日本語入力は既存の `with_japanese_ime.swift` から実行し、入力ソースを復元する。
- `insertion_empty_stale_acceptance.py` は0枚と古い要求の実行拒否を確認する。
- `insertion_reload_present_acceptance.py` は `--source` に保存済みの受け入れKDLを指定し、画像、再起動、両スライドの発表とネイティブ画像を確認する。
- `insertion_visual_acceptance.py` は `--appearance light` または `dark` で3つのウィンドウ寸法、表示と履歴、本文追加を確認する。
- `insertion_voiceover_acceptance.py` はVoiceOverを開始し、読み上げと追加を確認する。開始時はVoiceOverがオフであることを必要とし、終了時はscripting、welcome、起動状態を初期値へ復元する。

`cargo test --locked`、GUIバイナリの8テスト、`cargo fmt --check`、`git diff --check`、検証マップのlintは成功した。[coreログ](evidence/core-tests.log)、[GUIログ](evidence/gui-tests.log)。既存の公開文書APIによる全種類追加テストを維持し、`blank_slide_acceptance.py` と `input_scope_acceptance.py` の追加導線を新しいメニューへ移行し、両スクリプトも成功した。[白紙のログ](evidence/blank-slide.log)、[入力履歴のログ](evidence/input-scope.log)。

既存Issue #31のIME再変換と、#46の2画面Retina比較は対象外であり、修正済みとは扱わない。
