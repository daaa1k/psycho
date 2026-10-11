# MVP受け入れ条件と検証の対応

[親仕様 #17](https://github.com/daaa1k/psycho/issues/17)と[受け入れ #30](https://github.com/daaa1k/psycho/issues/30)の参照表。確認対象の基準コミットは`dda7563236f2dc4d6557df11231420e43240b436`。原文処理の修正は`5896122`、ネイティブメニューと再変換の修正を含む最終製品ソースは`8dc2ce3`。本表は追加テストと既存検証の対応を記録する。

「対応あり」はテスト本文または実機検証コードに該当する検査があることを示す。実行合格とは区別する。今回追加した公開操作テストの実行結果は末尾に記載する。既存の[evidence](evidence)と[README](README.md)の過去の合格記録は、現在のコミットの実機合格に読み替えない。今回の実機再実行の結果は[Issue #30の記録](issue-30.md)に記載する。

## チケットと保証

テスト名は省略せず記載する。特記のないRustテストは`tests/presentation_acceptance.rs`にある。GUIスクリプトは本ディレクトリーにある。

| チケット | 保証 | 公開操作のテスト | 実機で補う検証 |
| --- | --- | --- | --- |
| #19 | 原文の読込、無編集保存、値変更以外のバイト保持 | `save_preserves_bytes_and_value_edit_is_local`、`tests/presentation.rs`の`changing_a_value_replaces_only_its_source_token_and_undo_restores_bytes` | `no_edit_save_acceptance.py`、`sample_workflow_acceptance.py` |
| #19 | Schema全制約、部分表示の禁止 | `schema_constraints_are_reported_together_without_partial_preview`、`root_metadata_and_slide_constraints_are_schema_errors`、新規`schema_node_contracts_cover_unchecked_headers_children_and_empty_values` | `diagnostic_navigation_acceptance.py`でSchema診断の選択と読み取り専用ソース行を検査 |
| #19 | KDL構文、Unicode空白・改行、行継続、混在改行、再編集後の位置 | `valid_kdl_whitespace_and_continuations_survive_local_edits_and_moves`、`tests/diagnostics.rs`の`diagnostics_follow_every_kdl_newline_in_a_mixed_document`、`syntax_diagnostics_use_kdl_newlines_and_unicode_columns`、`diagnostics_after_successive_edits_use_the_reopened_source` | `diagnostic_navigation_acceptance.py`で構文診断の選択と読み取り専用ソース行を検査 |
| #19 | 書込み失敗、競合、原本保全 | `failed_write_keeps_original_edit_and_history_for_retry`、`tests/external_changes.rs`の`save_detects_invalid_external_source_and_freezes_history_without_polling` | `save_failure_acceptance.py`、`conflict_acceptance.py` |
| #20 | 入力単位、IME確定、欄内Undo、全体への履歴引継ぎ、Redo保持 | `history_limits_branching_and_same_value_operations`、`same_decoded_values_preserve_raw_quotes_empty_bullets_and_redo` | `ime_acceptance.py`、`ime_reconversion_acceptance.py`、`input_scope_acceptance.py`、`input_redo_transfer_acceptance.py`、`text_workflow_acceptance.py` |
| #20 | 1,000操作、保存後の履歴、保存バイトによる未保存判定 | `history_limits_branching_and_same_value_operations`、`save_preserves_bytes_and_value_edit_is_local` | `input_history_limit_acceptance.py`、`input_redo_transfer_acceptance.py` |
| #20 | 選択、直接編集、Delete、Escape、取消、無効入力、編集メニュー、閉じる3分岐 | 公開モデルだけではフォーカス・キー処理を証明できない | `text_workflow_acceptance.py`、`ime_acceptance.py`、`edit_menu_acceptance.py`、`close_cursor_acceptance.py` |
| #21 | Slide追加削除移動、Element追加削除移動、中身の復元 | `blank_slide_supports_all_elements_and_caption_and_rejects_nested_columns`、`inline_semicolon_nodes_can_reorder_at_every_boundary`、`tests/presentation.rs`の`moving_a_slide_preserves_source_and_keeps_inline_comments_with_it` | `blank_slide_acceptance.py`、`columns_workflow_acceptance.py` |
| #21 | コメント所属、引用表記、移動時の意味、挿入位置 | `move_into_and_out_of_columns_preserves_text_and_comment_ownership`、`moving_multiline_and_raw_strings_keeps_their_meaning`、`multiline_trailing_comments_move_with_the_element`、`root_to_column_moves_work_at_every_valid_insertion_position` | `columns_workflow_acceptance.py`で挿入表示とドラッグ1操作 |
| #22 | 空項目、項目追加削除移動、列内項目、原文と履歴 | `inline_bullets_reorder_at_every_boundary_without_moving_parent_content`、`nested_bullet_items_preserve_siblings_and_exact_history`、`same_decoded_values_preserve_raw_quotes_empty_bullets_and_redo` | `bullet_items_acceptance.py`、`direct_layout_acceptance.py` |
| #23 | コード本文・言語、タブ保存、引用形式 | 新規`prototype_08_01`、`prototype_08_02`、`moving_multiline_and_raw_strings_keeps_their_meaning`、`blank_slide_supports_all_elements_and_caption_and_rejects_nested_columns` | `text_workflow_acceptance.py`、`sample_workflow_acceptance.py`、`direct_layout_acceptance.py`、`code_colors_acceptance.py`でRust・YAML・JSON・TOMLと未対応言語・言語なし |
| #24 | PNG・JPEG・WebP、拡張子不一致、透過、EXIF、非対応・アニメーション・権限 | `image_formats_are_detected_by_content_and_missing_images_allow_editing`、`loaded_images_composite_transparency_on_white_and_apply_exif_orientation`、`asset_diagnostics_cover_animation_permissions_and_multiple_locations` | `assets_acceptance.py`で実画素と参照を検査 |
| #24 | 複数使用箇所、再読込失敗と回復、画像だけの更新、現在画像へのUndo | `asset_snapshots_share_failures_and_preserve_edits_and_history_during_recovery`、`image_path_history_loads_the_current_asset_bytes` | `diagnostic_navigation_acceptance.py`で画像診断からSlide・Image選択、`sample_workflow_acceptance.py`で画像参照編集と保存履歴 |
| #25 | 列内外移動、別Columns間移動、任意挿入位置、入れ子拒否 | `root_to_column_moves_work_at_every_valid_insertion_position`、`column_moves_change_only_the_requested_element_position`、`blank_slide_supports_all_elements_and_caption_and_rejects_nested_columns` | `columns_workflow_acceptance.py` |
| #25 | 1〜99・合計100、列幅ドラッグと入力検証 | `tests/presentation.rs`の`column_widths_change_together_and_invalid_totals_are_rejected`、Schemaの列幅ケース | `column_width_drag_acceptance.py`で境界ドラッグ・連続更新・1〜99制限・1回のUndo/Redo、`columns_workflow_acceptance.py`で数値・無効入力 |
| #26 | 6枚、全種、既定寸法、折返し、2列、画像・Caption・コード、編集2サイズと発表一致 | `six_slide_example_loads_with_all_assets_from_its_file_location`。実際の描画座標はモデルテストでは証明しない | `capture_layout.py`、`direct_layout_acceptance.py`、`presentation_layout_acceptance.py`、`following_layout_acceptance.py`、`measure_layout.py` |
| #26 | 画面外の全文修正、診断解除、はみ出し時の保存可・発表不可 | `tests/presentation_session.rs`の`empty_invalid_and_all_slide_layout_validation_prevent_start`は全Slideの検査接続を確認する。実際のGPUI文字組みの測定ではない | `layout_repair_acceptance.py`、`following_layout_acceptance.py` |
| #27 | 外部正常変更、競合、削除、無効化、入力だけの未保存、失敗後の履歴保持 | `tests/external_changes.rs`の`clean_external_change_reloads_and_clears_history`、`conflict_freezes_edits_and_history_until_explicit_reload`、`invalid_external_source_preserves_edits_and_diagnoses_the_disk_version`、`deletion_and_failed_retry_keep_the_document_and_both_history_branches`、`input_only_draft_prevents_auto_reload_and_failed_reload_keeps_redo` | `conflict_acceptance.py`、`presentation_external_change_acceptance.py` |
| #28 | 別フォルダーへの退避、元ファイル保全、画像パス補正、対象切替と履歴消去、失敗保持 | `failed_retreat_preserves_state_and_success_rebases_assets_and_clears_history`、`tests/retreat_save_as.rs`の`retreat_draft_failure_preserves_blocked_document_and_retry_adopts_source`、`retreat_preserves_symlink_asset_resolution_missing_assets_and_unrelated_bytes`、`retreat_survives_deleted_original_folder` | `save_as_retry_acceptance.py`、`assets_acceptance.py`、`conflict_acceptance.py`で既存先の確認と取消 |
| #29 | 両開始位置、未保存内容と画像の固定、全Slide事前検査、自動保存しない | `tests/presentation_session.rs`の`unsaved_snapshot_preserves_disk_history_and_navigation_bounds`、`current_start_follows_identity_across_clean_external_reload`、`empty_invalid_and_all_slide_layout_validation_prevent_start`、`external_conflict_invalid_and_deleted_files_prevent_start`、`image_pixels_are_frozen_after_update_and_delete`、`nested_images_on_other_slides_are_all_checked` | `presentation_workflow_acceptance.py`、`presentation_ime_acceptance.py` |
| #29 | 復帰位置のid・順番・末尾・0枚 | `tests/presentation_session.rs`の`position_falls_back_to_order_then_last_then_none`、`current_start_follows_identity_across_clean_external_reload` | `presentation_external_change_acceptance.py` |
| #29 | 全キー、端で停止、10回反復、別アプリ、フォーカス、終了キー、カーソル、クリック・スクロール無効 | 公開操作では実際のウィンドウ・キー配送・カーソルを証明できない | `presentation_workflow_acceptance.py --section keys`、`close_cursor_acceptance.py`、`presentation_ime_acceptance.py` |
| #30 | 代表例の文字・コード・列幅・画像・構成編集、保存、Undo/Redo、再起動、発表 | `six_slide_example_loads_with_all_assets_from_its_file_location`と各公開操作テスト | `sample_workflow_acceptance.py`。実読した構成移動ボタン名は`Element ↑`で、`src/main.rs`にも存在する |
| #30 | 白紙から全種追加、保存再読込、発表 | `blank_slide_supports_all_elements_and_caption_and_rejects_nested_columns` | `blank_slide_acceptance.py` |

安全な置換直前の競合について、`src/lib.rs`の`save_rechecks_changes_after_the_temporary_file_is_synced`と`save_as_rechecks_destination_changes_and_preserves_the_original_document`は、同期後の競合を注入する既存の内部単体テストである。公開操作の実ファイルケースと補い合うが、公開操作テストと呼び替えない。新規テストは非公開メソッドへ依存しない。

## Presentation Schema v0.1の全ノード

以下の「補完」は新規`tests/mvp_regression.rs`の`schema_node_contracts_cover_unchecked_headers_children_and_empty_values`を指す。「根」は`root_metadata_and_slide_constraints_are_schema_errors`、「要素」は`schema_constraints_are_reported_together_without_partial_preview`を指す。テスト名の対応は前表に記載した。

| ノード・横断条件 | 仕様の制約 | 対応 |
| --- | --- | --- |
| `presentation` | 文書内ちょうど1個、値・属性なし、先頭metadata1個、後ろslide0個以上 | 根で多重・値・属性・順序・欠落・未知子、補完で空文書・別の根と0枚の保存 |
| `metadata` | 値・属性なし、titleちょうど1個 | 根で値・属性・欠落・未知子、補完でtitle重複 |
| `title` | 空でない文字列1個、属性・子なし | 根で欠落・空・型・余分値・属性・子 |
| `slide` | 値なし、任意の文字列id0〜1個、許可した6種だけ、idは全体で一意 | 根で値・id型・未知属性・id属性重複・id値重複。補完で空idと全種の有効例 |
| `heading` | 文字列1個、空可、属性・子なし | 要素で欠落・属性、補完で型・余分値・子・空の有効例 |
| `text` | 文字列1個、空可、属性・子なし | 要素で型・余分値・子、補完で欠落・属性・空の有効例 |
| `bullets` | 値・属性なし、item0個以上だけ | 要素で値・未知子、補完で属性・空の有効例 |
| `item` | 文字列1個、空可、属性・子なし | 要素で型、補完で欠落・余分値・属性・子・空の有効例 |
| `code` | 文字列1個、空可、任意の文字列language0〜1個、未知属性・子なし | 要素でlanguage型・重複、補完で欠落・本文型・余分値・属性・子・空本文と空language |
| `image` | 空でない相対パス文字列1個、caption0〜1個だけ | 要素で空・絶対・型・caption重複・未知子、補完で欠落・余分値・属性・親参照の有効例 |
| `caption` | 文字列1個、空可、属性・子なし | 要素で型、補完で欠落・余分値・属性・子・空の有効例 |
| `columns` | 値・属性なし、columnちょうど2個 | 要素で欠落・1個・3個、補完で値・属性・未知子 |
| `column` | 値なし、整数width必須、各1〜99、合計100、許可した5種だけ | 要素でwidth欠落・0と100・小数・合計不正・属性重複・columns入れ子、補完で値・未知属性・文字列幅・負数・100超・未知子・境界1と99 |
| 同名属性、未知構文、型注釈 | 後勝ちを採用しない。未知ノード・属性・値・子・型違いを拒否 | 要素と根で同名属性、型注釈、未知ノード。補完で全種の属性と子の禁止を個別検査 |
| 無効文書の操作制限 | 部分プレビュー・編集・上書き保存不可、構文エラー時はSchema検査に進まない | `invalid_documents_are_diagnosed_and_cannot_edit_or_save`、診断テスト、新規補完でモデルなし・Schema分類・保存拒否・ディスク不変 |
| 0枚、画像問題 | 0枚は保存可・発表不可。画像欠落はSchemaと分離 | 補完の0枚保存、`empty_invalid_and_all_slide_layout_validation_prevent_start`、`image_formats_are_detected_by_content_and_missing_images_allow_editing` |

## 原文保持試作の10例・26操作

入力と期待するモデルは`7bc7ee7eac95b054a347521f753aae71b5a09dad`の`prototypes/kdl-preserving-edits/results.json`から採った。同コミットのREADMEと`src/main.rs`を読み、通常編集24操作、重複属性の診断実験1操作、意味が変わる移動候補の棄却実験1操作を区別した。

新規`tests/mvp_regression.rs`は通常編集24操作と診断実験1操作を独立したテストにした。各通常編集は元記録の操作直前のKDLを実ファイルに置き、製品の公開操作を呼ぶ。期待する原文は独立した文字列リテラルで定義し、モデルに加えて原文全体を比較する。保存ファイル、再読込、Redoも同じ期待原文と照合する。Undoでは操作前の全バイト復元とディスクが変わらないことを検査する。コメント、slashdash、未編集範囲の欠落はモデルが一致しても失敗する。

試作から期待値を変えたのは仕様で認める箇所だけである。例5・6の移動では引用された複数行文字列内部の字下げを元のバイト列に戻した。移動先に合わせるのは引用外の字下げである。例8の言語値変更は製品が出力する通常引用を期待する。属性名・値の前後のコメントとslashdash属性はそのまま残す。周囲にタブの手掛かりがある例7・10ではタブを期待し、4空白への置換を許容しない。

| 例 | 元記録の各操作と新規テスト |
| --- | --- |
| 1 無編集保存と値変更 | 無編集保存`prototype_01_01`、本文変更`prototype_01_02`、画像パス変更`prototype_01_03` |
| 2 コメントを伴う移動と削除 | 画像を左列へ`prototype_02_01`、列内画像削除`prototype_02_02`、同じ行の2番目の本文を右列へ`prototype_02_03` |
| 3 通常文字列 | コードを左列へ`prototype_03_01`、列外へ`prototype_03_02` |
| 4 生文字列 | コードを左列へ`prototype_04_01`、列外へ`prototype_04_02` |
| 5 複数行文字列 | コードを左列へ`prototype_05_01`、列外へ`prototype_05_02` |
| 6 生の複数行文字列 | コードを左列へ`prototype_06_01`、列外へ`prototype_06_02` |
| 7 追加と周囲の書式 | 末尾追加`prototype_07_01`、空Slideへ追加`prototype_07_02`、先頭追加`prototype_07_03`。製品の追加は末尾のため、先頭追加は公開の追加と移動を組み合わせる |
| 8 連続編集後の診断 | 言語変更`prototype_08_01`、本文変更`prototype_08_02`、本文追加`prototype_08_03`、重複language診断`prototype_08_04`。連続編集後の現在原文との診断位置照合は`tests/diagnostics.rs`の`diagnostics_after_successive_edits_use_the_reopened_source` |
| 9 一律インデントの反例 | 棄却実験は試作内部の候補生成を前提とし、対応する製品公開操作がない。既存内部テスト`move_candidate_rejects_valid_kdl_with_unexpected_meaning_without_changing_history`が候補拒否を検査する。元の反例入力を製品公開操作で意味を保持して移動する2操作は`prototype_09_02`、`prototype_09_03` |
| 10 文字列とコメントの区別 | CRLF・タブ・入れ子コメント・slashdash生複数行文字列を左列へ`prototype_10_01`、列外へ`prototype_10_02` |

26操作すべてに検証との対応を記したが、棄却実験まで公開操作で再演したとは主張しない。KDL 2.0全体への適合証明、全入力の全挿入位置の網羅、性能測定にも当たらない。

## 検証結果と残る境界

追加テストは以下で実行した。

```sh
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang \
  CC=/usr/bin/clang CXX=/usr/bin/clang++ \
  cargo test --locked --test mvp_regression
```

全27テストの内訳は元試作の通常編集24件、診断実験1件、Schemaヘッダー・子・空値・根の補完1件、共有画像削除時のファイル保持と履歴1件である。厳密な原文比較で再現した4件を修正し、27件すべてが合格した。移動したコメント・閉じ括弧、同一行から移動した要素、空の親への追加で周囲のタブを使う処理を修正した。失敗を許容する期待値変更はしていない。

`tests/moved_trivia.rs`の13件は、引用内のエスケープ付き改行、コメントと括弧のタブ、全8種類のKDL改行によるコメント終端・削除・移動・属性挿入・Caption追加・列内外の往復を公開操作で検査する。引用内にある7種類の非LF改行を文書の改行様式に使わないことも、原文・保存ファイル・再読込・Undo/Redoで検査する。独立レビューが見つけた引用内改行の誤認を修正した。

`8dc2ce3`の`cargo test --locked --features gui`は[全112件成功](evidence/issue-30/history/tests-gui-native-final.log)した。GUIを有効にしないテストは109件である。GUI固有の3件には内部境界の検査も含まれ、112件すべてを公開操作テストとは呼ばない。

実機検証コードの存在だけではIME、描画、入力焦点、全画面、カーソルの合格を証明できない。今回の実行結果、リビジョン、環境、録画、比較画像と座標ログは[Issue #30の記録](issue-30.md)で個別に示す。保存競合の注入テストは競合窓の特定地点を検査し、あらゆるOS障害時点の証明ではない。
