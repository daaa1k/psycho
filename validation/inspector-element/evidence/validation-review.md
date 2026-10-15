# 最終検証スクリプトの独立レビュー

ブロッキング指摘は1件。

## VoiceOver の終了処理がアプリの生存に依存し、失敗時にユーザー設定を残す

`validation/inspector-element/voiceover.py:155` の `gui.act('get-app-state', '--restore-window')` が失敗すると、VoiceOver の停止と scripting/welcome の復元、起動した VoiceOver Utility の終了をすべて飛ばす。アプリがクラッシュした場合や Orca が対象アプリを見つけられない場合に発生する。外側の finally は専用アプリの終了と observations の保存しか行わないため、VoiceOver がオンのまま、scripting=true と welcome=false が残る。

現行ファイルの finally を AST から抽出し、VoiceOver 有効、元設定取得済み、gui.act が app_not_found を投げる条件で実行した。GUI や実設定は変更していない。実際の呼び出し記録は `['restore-window', 'stop-owned-app', 'write-observations']` だった。設定復元も VoiceOver Utility 終了も実行されなかった。

VoiceOver の停止を対象アプリから独立して実行し、それが失敗しても各設定の復元と Utility の終了を個別に必ず試みる必要がある。最後に現在値を確認し、復元失敗は検証失敗として報告する。

## 確認範囲

新規の validation/inspector-element/*.py、native_identity.swift、mvp の inspector_element_acceptance.py、scroll_at.swift、gui_session.py の差分、verify feature と README を読んだ。IME wrapper の復元処理も読んだ。ソース編集、GUI操作、ビルドは行っていない。

保存結果、ネイティブAXフォーカス、VoiceOverの実操作、IME候補ウインドウの座標を使う検証に、上記以外の具体的なブロッキング欠陥は見つからなかった。実行中の最終ランは未確認なので成功判定には含めていない。新規スクリプトに操作段階を説明するコメントはなかった。READMEにない部分実行フラグはあるが、通常実行の検証を弱めるものではない。

Prove It Works を適用し、復元漏れをコードの推測だけでなく、実ファイルから抽出した終了処理の呼び出し記録で確認した。no-comments と deslop の観点を読み取りレビューに適用した。親からの明示的な禁止に従い、追加のサブエージェントは起動していない。

## 修正後の差分レビュー

前述のブロッキング指摘は解消した。現行 finally に gui.act への依存はなく、VoiceOver 停止、scripting 復元、welcome 復元、実値確認、Utility 終了、専用アプリ終了、観測保存を個別に試みる。失敗は最後に RuntimeError へ集約する。

修正後の実ファイルから finally を AST 抽出し、VoiceOver 停止、scripting 復元、実値確認、専用アプリ終了の各段階がそれぞれ失敗する4条件と、全成功する1条件で実行した。全条件で後続の独立した終了処理を実行した。失敗4条件はすべて最終 RuntimeError となり、成功条件だけ正常終了した。設定復元の1件目が失敗しても2件目と実値確認、Utility 終了、専用アプリ終了、観測保存まで実行することを確認した。

差分レビューの判定はブロッキング指摘なし。これは終了処理の制御フローの確認であり、osascript による実際の VoiceOver 停止の成功は親の実機ランで確認する必要がある。GUI、実設定、ソースは変更していない。

## ネイティブ停止ヘルパー追加後の差分レビュー

`stop_voiceover.swift` と呼び出し側を確認した。ブロッキング指摘なし。元のPSYCHOアプリを参照せず、VoiceOver有効状態を確認してからグローバルのFn+Command+F5を送る。Quickstartがある場合はそのアプリのAXツリーだけから明示的なVoiceOver停止ボタンを探す。停止後はNSWorkspaceの実値を最大6秒確認し、有効のままなら異常終了する。既にオフの場合はショートカットを送らない。対象と状態確認の選び方は今回の終了処理に適切である。

最新のPython finallyを再度AST抽出し、停止ヘルパー失敗、scripting復元失敗、正常終了の3条件で実行した。いずれも両設定の復元、実値確認、Utility終了、専用アプリ終了、観測保存まで試行した。失敗条件では最終RuntimeErrorを確認した。実GUIやユーザー設定には触れていない。

前回記載したosascript単独停止の実機成功は成立しなかったと親から報告を受けた。そのため、前回の制御フロー承認を停止手段の実機承認として扱わない。新ヘルパーの実機動作は親が実行中の設定編集ランとrestoration.jsonで別途確認する。
