"""Exercise Issue #27 through Orca Computer Use against an owned scratch file."""
import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time
import unicodedata

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "mvp-acceptance"))
from gui_session import Gui as BaseGui, root


class Gui(BaseGui):
    def act(self, *args):
        try:
            return super().act(*args)
        except AssertionError as error:
            detail = error.args[0] if error.args else {}
            if (args[0] != "get-app-state" or not isinstance(detail, dict)
                    or detail.get("error", {}).get("code") != "app_not_found"
                    or self.app.poll() is not None):
                raise
            # The provider can transiently miss a live, unbundled process.
            # Re-enumerate before retrying this read-only observation once.
            apps = json.loads(subprocess.check_output(
                ["orca", "computer", "list-apps", "--json"], text=True))
            assert apps["ok"] and any(app["pid"] == self.app.pid for app in apps["result"]["apps"]), apps
            return super().act(*args)

SOURCE = 'presentation {\n metadata { title "External recovery" }\n slide id="intro" { heading "Original heading" }\n}\n'


def normalized(text):
    return unicodedata.normalize("NFKC", "".join(text.split()))


def run(gui):
    records = []

    def observe(name, *expected):
        state, rows = gui.snapshot(name)
        assert state["screenshot"]["scale"] == 1
        text = normalized(" ".join(row["text"] for row in rows))
        record = dict(name=name, text=text, input=gui.state(), window=state["snapshot"]["window"],
                      tree=state["snapshot"]["treeText"], expected=list(expected))
        records.append(record)
        (gui.out / "observations.json").write_text(json.dumps(records, ensure_ascii=False, indent=2) + "\n")
        for fragment in expected:
            assert normalized(fragment) in text, (name, fragment, text)
        print("PASS", name, flush=True)
        return state, rows

    def edit_heading(text):
        state, rows = gui.snapshot()
        assert state["screenshot"]["scale"] == 1
        row = next(row for row in rows if row["text"] == text and 160 < row["x"] < 995)
        gui.click(row["x"], row["y"], 2)
        assert gui.state()["focused"], "Canvas input did not receive focus"

    def reload_dirty():
        gui.press("外部変更を読み込む")
        _, rows = gui.snapshot()
        # Vision occasionally misreads 棄 or omits 再 in this narrow button.
        # Locate the visible confirmation by its unique toolbar prefix.
        row = next(row for row in rows if row["y"] < 100 and row["text"].startswith("編集を"))
        gui.click(row["x"], row["y"])

    def source_line():
        _, rows = gui.snapshot()
        row = next(row for row in rows if "構文" in row["text"] or "Schema" in row["text"])
        gui.click(row["x"], row["y"])

    clean = SOURCE.replace("Original heading", "External clean")
    gui.fixture.write_text(clean)
    time.sleep(1)
    observe("01-clean-auto-reload", "External clean", "再読み込みしました")
    edit_heading("External clean")
    gui.replace("Local committed")
    gui.key("Escape")
    edit_heading("Local committed")
    gui.replace("Retained draft")
    gui.replace("Future draft")
    gui.hotkey("CmdOrCtrl+Z")
    retained = gui.state()
    assert retained["text"] == "Retained draft" and retained["undo"] > 0 and retained["redo"] > 0
    external = SOURCE.replace("Original heading", "External conflict")
    gui.fixture.write_text(external)
    time.sleep(1)
    observe("02-conflict", "競合", "Local committed", "Retained draft", "一致していません")
    for key in ["CmdOrCtrl+S", "CmdOrCtrl+Z", "CmdOrCtrl+Shift+Z"]:
        gui.hotkey(key)
    gui.press("最初から発表")
    gui.press("現在から発表")
    state, rows = observe("03-conflict-frozen", "Local committed", "Retained draft")
    assert state["screenshot"]["width"] == 1280
    for field in ["text", "undo", "redo"]:
        assert gui.state()[field] == retained[field], (field, gui.state(), retained)
    assert not gui.state()["focused"]
    assert gui.fixture.read_text() == external
    gui.press("外部変更を読み込む")
    observe("04-discard-confirmation", "未保存の編集内容を破棄します")
    gui.press("読み込みをやめる")
    assert gui.state()["text"] == "Retained draft"

    gui.fixture.write_text("presentation {")
    reload_dirty()
    observe("05-syntax-failed-retry", "KDL", "Local committed", "一致していません")
    for field in ["text", "undo", "redo"]:
        assert gui.state()[field] == retained[field]
    source_line()
    observe("06-syntax-readonly-source", "読み取り専用", "presentation {")
    assert gui.fixture.read_text() == "presentation {"

    schema = SOURCE.replace('heading "Original heading"', 'mystery "Invalid element"')
    gui.fixture.write_text(schema)
    reload_dirty()
    observe("07-schema-failed-retry", "Schema", "Local committed")
    source_line()
    observe("08-schema-readonly-source", "読み取り専用", "mystery", "Invalid element")
    assert gui.state()["text"] == "Retained draft"
    assert gui.fixture.read_text() == schema

    gui.fixture.unlink()
    reload_dirty()
    observe("09-deleted-failed-retry", "削除", "Local committed", "一致していません")
    gui.hotkey("CmdOrCtrl+S")
    assert not gui.fixture.exists()
    for field in ["text", "undo", "redo"]:
        assert gui.state()[field] == retained[field]

    recovered = SOURCE.replace("Original heading", "Recovered heading")
    gui.fixture.write_text(recovered)
    reload_dirty()
    observe("10-recovered", "Recovered heading", "再読み込みしました")
    assert gui.state()["undo"] == 0 and gui.state()["redo"] == 0
    gui.hotkey("CmdOrCtrl+Z")
    gui.hotkey("CmdOrCtrl+Shift+Z")
    gui.hotkey("CmdOrCtrl+S")
    assert gui.fixture.read_text() == recovered
    gui.press("最初から発表")
    state, _ = observe("11-presentation-restored", "Recovered heading")
    assert state["screenshot"]["width"] > 1280
    gui.key("Escape")
    edit_heading("Recovered heading")
    gui.replace("After recovery")
    gui.hotkey("CmdOrCtrl+S")
    assert 'heading "After recovery"' in gui.fixture.read_text()
    observe("12-edit-save-restored", "After recovery")
    saved = gui.fixture.read_text()
    gui.fixture.unlink()
    time.sleep(1)
    observe("13-clean-deletion", "削除", "After recovery", "一致していません")
    gui.hotkey("CmdOrCtrl+S")
    assert not gui.fixture.exists()
    gui.fixture.write_text(saved)
    gui.press("外部変更を読み込む")
    observe("14-clean-deletion-recovered", "After recovery", "再読み込みしました")
    gui.fixture.write_text("presentation {")
    time.sleep(1)
    observe("15-clean-invalid", "KDL", "After recovery", "一致していません")
    source_line()
    observe("16-clean-invalid-readonly", "読み取り専用")
    gui.fixture.write_text(saved)
    gui.press("外部変更を読み込む")
    observe("17-clean-invalid-recovered", "After recovery", "再読み込みしました")

    edit_heading("After recovery")
    gui.replace("Retreat draft")
    gui.replace("Future retreat")
    gui.hotkey("CmdOrCtrl+Z")
    before_retreat = gui.state()
    assert before_retreat["text"] == "Retreat draft" and before_retreat["redo"] > 0
    gui.fixture.write_text(external)
    time.sleep(1)
    observe("18-retreat-conflict", "競合", "Retreat draft")
    archive = gui.out / "archive"
    archive.mkdir()
    destination = archive / "presentation.kdl"
    mode = stat.S_IMODE(archive.stat().st_mode)

    def choose_destination():
        gui.press("別名保存")
        state = gui.act("get-app-state")
        row = next(row for row in state["snapshot"]["treeText"].splitlines()
                   if row.strip().endswith("button 退避先を選ぶ"))
        gui.act("click", "--element-index", row.strip().split()[0], "--no-screenshot")
        gui.hotkey("CmdOrCtrl+Shift+G")
        state = gui.act("get-app-state")
        assert "text" in state["snapshot"]["treeText"].lower()
        gui.hotkey("CmdOrCtrl+A")
        gui.act("type-text", "--text", str(archive), "--no-screenshot")
        gui.key("Return")
        state = gui.act("get-app-state")
        assert "Where:, Value: archive" in state["snapshot"]["treeText"], state

    def native_save():
        state = gui.act("get-app-state")
        row = next(row for row in state["snapshot"]["treeText"].splitlines()
                   if row.strip().endswith("button Save"))
        gui.act("click", "--element-index", row.strip().split()[0], "--no-screenshot")

    try:
        choose_destination()
        os.chmod(archive, 0o500)
        native_save()
        observe("19-retreat-failed", "退避保存できません", "Retreat draft")
        assert gui.fixture.read_text() == external and not destination.exists()
        for field in ["text", "undo", "redo"]:
            assert gui.state()[field] == before_retreat[field]
        gui.hotkey("CmdOrCtrl+Z")
        gui.hotkey("CmdOrCtrl+Shift+Z")
        for field in ["text", "undo", "redo"]:
            assert gui.state()[field] == before_retreat[field]
        os.chmod(archive, mode)
        choose_destination()
        native_save()
        observe("20-retreat-recovered", "別ファイルへ保存しました", "Retreat draft")
        assert 'heading "Retreat draft"' in destination.read_text()
        assert gui.fixture.read_text() == external
        assert gui.state()["undo"] == 0 and gui.state()["redo"] == 0
        contents = destination.read_bytes()
        gui.hotkey("CmdOrCtrl+Z")
        gui.hotkey("CmdOrCtrl+Shift+Z")
        gui.hotkey("CmdOrCtrl+S")
        assert destination.read_bytes() == contents and gui.fixture.read_text() == external
        edit_heading("Retreat draft")
        gui.replace("Archive edited")
        gui.hotkey("CmdOrCtrl+S")
        assert 'heading "Archive edited"' in destination.read_text()
        assert gui.fixture.read_text() == external
        observe("21-retreat-edit-save", "Archive edited")
    finally:
        os.chmod(archive, mode)
    return records


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--case", choices=["all", "diagnostics"], default="all")
    args = parser.parse_args()
    gui = Gui(args.output, SOURCE)
    try:
        if args.case == "diagnostics":
            gui.fixture.write_text(SOURCE.replace('heading "Original heading"', 'mystery "Invalid element visible_tail"'))
            time.sleep(1)
            state, rows = gui.snapshot("diagnostic-width")
            text = normalized(" ".join(row["text"] for row in rows))
            assert "visible_tail" in text, text
            print("PASS entire diagnostic source line is visible", flush=True)
        else:
            run(gui)
    except Exception:
        gui.snapshot("failure")
        raise
    finally:
        gui.stop()
