# #56 検証ドライバーの独立レビュー

検証変更の主要部分は妥当です。製品仕様全体の受け入れ完了判定ではありません。GUI と cargo は実行せず、差分、スクリプト、保存済み証跡を読みました。

レビュー対象 HEAD は `26259cabc1c96ce2a6a04b4f2fb702766c3cf4ff`。`06eb12e` から作業ツリーまでの validation 差分 SHA-256 は `0df0b0dfe82eb050a7245331cd4d374969af4d24f7605dff64833d020d40cf18`。README と driver-design の未追跡ファイルも読みました。

- P2。`validation/editor-acceptance/collect.py:77` は `launch-*.json`、各 snapshot JSON、`observation-recovery.jsonl` を収集しません。実際に accepted/sample に4起動、accepted/blank に2起動の記録があり、それぞれの PID と binary SHA はここにあります。`native-input.jsonl` の PID とバイナリーの対応をアーカイブ単体で追えなくなります。最低限 launch 記録を追加し、失敗調査に必要な snapshot と recovery 記録も保持してください。選択した画像に対応する AX JSON も保存すると、OCR と AX の根拠を後から点検できます。
- 既存の弱い検査。`validation/mvp-acceptance/measure_layout.py:81-83` の `quant > 1` は有限座標では成立しません。floor との差は定義上 x で0.25未満、yで1未満です。座標一致と raster stroke 比較は別に実在し有効ですが、最終 PASS に含む「quantization」を実測検証済みと解釈してはいけません。今回導入された回帰とは判定しません。
- 限定事項。白紙レシピの画像スライドは保存された相対パスと Caption の OCR を検査しますが、画像ピクセル自体の表示は判定しません。画像非表示でも Caption が出ればこのケース単体は通ります。画像描画は別の画像検証結果か保存スクリーンショットの目視を根拠にしてください。sample の画像変更も同一内容を複製した updated.png を使うため、パス更新は証明しますが、異なる絵への更新は証明しません。

`Gui.focus_input` と Swift は、PID、実行ファイル、前面アプリ、完全一致する欄、所属ウィンドウ、クリック後の同じ AX 要素のフォーカスを確認しています。キー送信 receipt は配送結果として扱われ、sample/blank は保存 KDL の期待値、全バイト Undo/Redo、再起動後の編集欄と保存値を別に検査します。この境界で新たな重大な偽陽性は見つかりませんでした。

実ファイルも読みました。accepted/sample/presentation.kdl は2039 bytesで変更後タイトルを保持し、11段階の保存観測があります。accepted/blank/presentation.kdl は529 bytesで Authored presentation を保持し、0枚から復帰まで6段階の観測があります。起動記録の binary SHA はすべて `9b9d5ba711f53cafb18d88600079938845c30ca6c5e93053c1e05414b0e10a93` です。起動記録の Git HEAD は `cebb8367fe801301953f3614ad864f4a55cc43ab` であり、製品 revision `06eb12e` と区別する README の説明は妥当です。

確認時の accepted/results.json は sample、blank、document-status、document-present の4件で returncode 0 でした。VoiceOver の再実行完了や #56 全条件の合格はまだこのファイルから言えません。既知 #77 は意図した FAIL として残してください。collect.py は verdict を作らず元の結果を収集するため、古い失敗を新しい合格へ書き換える挙動はありません。

適用した Prove It Works は、配送成功を合格根拠にせず、実際の保存ファイルと起動記録を読む判断に使いました。unslop に従い、検証で確認できた範囲と未確認範囲を分けて記述しました。

追記。親エージェントは launch 記録と recovery 記録の収集を追加する方針を採用しました。snapshot 全体は保存パネルのファイル一覧を含むため非公開とし、選択画像・状態・観測記録で代替する方針です。量子化の合格主張も外すとの回答でした。変更後コードの再レビューはこの報告には含みません。

Overview の実 snapshot も照合しました。sample-overview に `list Overviewのスライド一覧` があり、前後の sample-structure と sample-slide-move-earlier には Overview 文字列がありません。この保存済み実行について、Overview 往復確認の偽陽性は認めません。
