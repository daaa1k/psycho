# 再現と修正の経過

最終製品の合否は[受け入れ記録](../../../issue-30.md)にある。ここには修正前の失敗も保管する。決定ログの`target/issue30`と`/tmp`のパスは実行時の出力先であり、以下のファイルが保存版である。

| 検証 | 失敗記録 | 修正後・最終記録 |
| --- | --- | --- |
| 旧試作の厳密原文 | [regression-strict.log](regression-strict.log)、[regression-strengthened.log](regression-strengthened.log) | [regression-fixed.log](regression-fixed.log) |
| 引用内Unicode改行 | [psycho-quoted-newline-before.log](psycho-quoted-newline-before.log) | [psycho-quoted-newline-all.log](psycho-quoted-newline-all.log) |
| 引用内改行と移動先様式 | [structural-newline-red.log](structural-newline-red.log) | [tests-gui-final2.log](tests-gui-final2.log) |
| nativeメニューの名前 | [native-menu-no-edit.log](native-menu-no-edit.log) | [native-menu-fixed.log](native-menu-fixed.log) |
| nativeメニューの無効状態 | [native-menu-enabled-red.log](native-menu-enabled-red.log) | [native-menu-fixed.log](native-menu-fixed.log) |
| 標準IME再変換のUndo | [ime-reconversion-red.log](ime-reconversion-red.log)、[観測値](ime-reconversion-red.json)、[実コールバック](ime-reconversion-callbacks.log) | 最終実行の`ime-reconversion.log`と保存原文を受け入れ記録から参照する |
| 最終製品`8dc2ce3` | 該当なし | [GUIなし109件](tests-public-native-final.log)、[GUI有効112件](tests-gui-native-final.log) |

字下げ設計の[Sol案](indent-design-sol.md)と[Luna案](indent-design-luna.md)は比較時の記録であり、採用結果は決定ログと製品ソースにある。元試作との差分のうち、引用内バイト4件と変更値の引用形式1件だけを仕様に合わせた。残る原文の不一致4件は修正前に失敗する検査として保持した。
