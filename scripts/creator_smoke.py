#!/usr/bin/env python3
"""Create a body, a voxel hat and a character through the 1280×720 editor UI.

Everything is driven with real X input against the shipped binary. Control
bounds come from the --state-file manifest; nothing is injected.
"""
import argparse
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import time

from PIL import Image, ImageChops, ImageGrab, ImageStat
from visual_support import ROOT, SCREEN_SIZE, game_binary, software_vulkan, start_xvfb, stop, wait_for


def float32_model(value):
    """Compare f32 JSON values even when serializers choose different precision."""
    if isinstance(value, float):
        return struct.unpack("!f", struct.pack("!f", value))[0]
    if isinstance(value, dict):
        return {key: float32_model(item) for key, item in value.items()}
    if isinstance(value, list):
        return [float32_model(item) for item in value]
    return value


class Session:
    def __init__(self, env, output, data, report, workspace=None, prefix="game", size=SCREEN_SIZE, extra=()):
        self.env, self.output, self.report, self.size = env, output, report, size
        self.state_path = output / f"{prefix}-state.json"
        self.state_path.unlink(missing_ok=True)
        self.capture_number = 0
        self.prefix = prefix
        self.f12 = output / f"{prefix}-f12"
        ready_capture = output / f"{prefix}-ready.png"
        command = [str(game_binary()), "--mode", "editor", "--data-dir", str(data),
                   "--state-file", str(self.state_path), "--output-dir", str(self.f12),
                   "--capture", str(ready_capture), "--capture-frame", "120", *extra]
        if workspace:
            command += ["--editor-view", workspace]
        self.log_path = output / f"{prefix}.log"
        with self.log_path.open("w") as log:
            self.process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            self.wait(lambda s: bool(s.get("controls")), "initial control manifest")
            wait_for(lambda: f"capture_saved path={ready_capture}" in self.log_path.read_text(),
                     "initial renderer warmup", self.process)
            self.window = self.xdo("search", "--sync", "--onlyvisible", "--pid", self.process.pid).splitlines()[0]
            self.xdo("windowfocus", "--sync", self.window)
            geometry = self.geometry()
            assert (geometry[2], geometry[3]) == size, geometry
        except Exception:
            stop(self.process)
            raise

    def xdo(self, *arguments):
        return subprocess.check_output(["xdotool", *map(str, arguments)], env=self.env, text=True, timeout=10).strip()

    def geometry(self):
        values = dict(line.split("=", 1) for line in self.xdo("getwindowgeometry", "--shell", self.window).splitlines())
        return tuple(int(values[key]) for key in ("X", "Y", "WIDTH", "HEIGHT"))

    def state(self):
        try:
            return json.loads(self.state_path.read_text())
        except (OSError, ValueError):
            return {}

    def wait(self, predicate, description, timeout=120):
        wait_for(lambda: predicate(self.state()), description, self.process, timeout=timeout)
        return self.state()

    def settle(self):
        """Wait for a fresh manifest written after the latest rebuild."""
        revision = self.state().get("revision", 0)
        time.sleep(0.15)
        return self.wait(lambda s: s.get("revision", 0) >= revision and bool(s.get("controls")), "settle")

    def control(self, match, by="action"):
        state = self.settle()
        found = [c for c in state["controls"] if match(c[by])]
        if not found:
            found = self.reveal(match, by)
        assert found, f"No control matched in {by}: available {sorted(set(c[by] for c in state['controls']))}"
        return found[0]

    def reveal(self, match, by):
        """Wheel-scroll the inspector, outline and explorer looking for a control."""
        order = ("popup", "inspector", "outline", "explorer")
        panes = sorted(self.state().get("panes", []),
                       key=lambda p: next((i for i, k in enumerate(order) if p["key"].startswith(k)), 9))
        for pane in panes:
            if pane["max"] <= 0:
                continue
            x, y, width, height = pane["rect"]
            self.xdo("mousemove", "--sync", "--window", self.window, round(x + width / 2), round(y + height / 2))
            # Long lists (e.g. a 100-tile dropdown) need more than a few notches.
            notches = max(12, round(pane["max"] / 40) + 4)  # 40 px per wheel notch
            for button in (5,) * notches + (4,) * (notches * 2):
                self.xdo("click", button)
                time.sleep(0.12)
                found = [c for c in self.state()["controls"] if match(c[by])]
                if found:
                    time.sleep(0.2)
                    return [c for c in self.state()["controls"] if match(c[by])]
        return []

    def click(self, x, y, button=1):
        assert 0 <= x < self.size[0] and 0 <= y < self.size[1], (x, y)
        self.xdo("mousemove", "--sync", "--window", self.window, round(x), round(y))
        time.sleep(0.07)
        self.xdo("mousedown", button)
        time.sleep(0.15)
        self.xdo("mouseup", button)

    def press(self, control, description, button=1, expect_change=True):
        before = self.state()["revision"]
        x, y = control["center"]
        width, height = control["size"]
        assert width > 0 and height > 0, control
        self.click(x, y, button)
        if expect_change:
            after = self.wait(lambda s: s.get("revision", 0) > before, description)
        else:
            time.sleep(0.3)
            after = self.state()
        time.sleep(0.08)
        self.report["actions"].append({"action": description, "revision": after["revision"], "notice": after.get("notice")})
        if len(self.report["actions"]) % 20 == 0:
            print(f"Progress: {len(self.report['actions'])} UI actions; {description}; revision {after['revision']}", flush=True)
        if after.get("notice_kind") == "Error":
            raise AssertionError(f"{description}: {after['notice']}")
        errors = [line for line in self.log_path.read_text().splitlines()
                  if (" ERROR " in line and "gilrs" not in line) or "panicked at" in line]
        if errors:
            raise AssertionError(f"Renderer error after {description}: {errors[-1]}")
        return after

    def action(self, action):
        """Click the first visible control whose action is exactly `action`."""
        return self.press(self.control(lambda a: a == action), action)

    def action_starting(self, prefix):
        return self.press(self.control(lambda a: a.startswith(prefix)), prefix)

    def menu(self, opener, item):
        self.action(opener)
        self.wait(lambda s: s.get("menu") is not None, f"menu {opener}")
        # Pick the menu entry itself, not a control with the same action beneath it.
        state = self.settle()
        found = [c for c in state["controls"] if c["action"] == item and c["kind"] == "MenuItem"]
        if not found:
            found = [c for c in self.reveal(lambda a: a == item, "action") if c["kind"] == "MenuItem"]
        assert found, f"No menu item {item}: {[c['action'] for c in state['controls'] if c['kind'] == 'MenuItem']}"
        return self.press(found[0], item)

    def new_resource(self, kind):
        self.menu("OpenMenu(New)", f"New({kind})")
        self.wait(lambda s: (s.get("editing") or "").startswith("DocName"), "new resource name entry")

    def type_and_commit(self, text):
        # A field that starts editing has its whole value selected: typing replaces it.
        self.xdo("type", "--delay", 40, "--", text)
        self.wait(lambda s: s.get("naming") == text, f"typed {text!r}")
        self.xdo("key", "Return")
        self.wait(lambda s: s.get("editing") is None, "edit committed")

    def edit_text(self, target, text):
        self.action(f"Edit({target})")
        self.wait(lambda s: s.get("editing") == target, f"editing {target}")
        self.type_and_commit(text)

    def field(self, field, value):
        """Click a numeric field (no drag) to type an exact value."""
        control = self.control(lambda k: k == f"Field({field})", by="kind")
        self.press(control, f"type {field}")
        self.wait(lambda s: s.get("editing") == f"Value({field})", f"editing {field}")
        self.type_and_commit(f"{value:g}" if isinstance(value, float) else str(value))
        if self.state().get("notice_kind") == "Error":
            raise AssertionError(self.state()["notice"])

    def scrub(self, field, dx):
        """Drag horizontally across a numeric field to scrub its value."""
        control = self.control(lambda k: k == f"Field({field})", by="kind")
        before = self.state()["revision"]
        x, y = control["center"]
        self.xdo("mousemove", "--sync", "--window", self.window, round(x), round(y))
        time.sleep(0.07)
        self.xdo("mousedown", 1)
        for step in range(1, 11):
            self.xdo("mousemove", "--sync", "--window", self.window, round(x + dx * step / 10), round(y))
            time.sleep(0.05)
        time.sleep(0.15)
        self.xdo("mouseup", 1)
        self.wait(lambda s: s.get("revision", 0) > before and s.get("editing") is None, f"scrub {field}")
        time.sleep(0.2)
        self.report["actions"].append({"action": f"scrub {field} by {dx}px", "revision": self.state()["revision"]})

    def drag_handle(self, handle, part, dx, dy):
        """Drag a viewport move handle by real mouse input."""
        state = self.settle()
        found = [h for h in state.get("handles", []) if h["handle"].startswith(handle) and h["part"] == part]
        assert found, f"No handle {handle} {part}: {state.get('handles')}"
        before = state["revision"]
        x, y = found[0]["center"]
        self.xdo("mousemove", "--sync", "--window", self.window, round(x), round(y))
        time.sleep(0.3)
        self.xdo("mousedown", 1)
        for step in range(1, 11):
            self.xdo("mousemove", "--sync", "--window", self.window, round(x + dx * step / 10), round(y + dy * step / 10))
            time.sleep(0.05)
        time.sleep(0.15)
        self.xdo("mouseup", 1)
        self.wait(lambda v: v.get("revision", 0) > before, f"drag {handle} {part}")
        time.sleep(0.3)
        self.report["actions"].append({"action": f"drag handle {handle} {part}", "revision": self.state()["revision"]})

    def library(self):
        return self.settle()["library"]

    def viewport_paint(self):
        before = self.library()
        x0, y0, width, height = self.state()["viewport"]
        for fx, fy in ((0.5, 0.75), (0.45, 0.7), (0.55, 0.8), (0.4, 0.6), (0.6, 0.65), (0.5, 0.55)):
            x, y = x0 + width * fx, y0 + height * fy
            self.click(x, y)
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                if self.state().get("library") != before:
                    self.report["viewport_paint"] = {"position": [x, y], "revision": self.state()["revision"]}
                    self.capture("viewport-painted")
                    self.action("Undo")
                    assert self.library() == before
                    return
                time.sleep(0.1)
        raise AssertionError("No direct viewport click painted a voxel")

    def capture(self, name, check_viewport=True, neutral=False):
        # Park the pointer where it cannot hover a control.
        x0, y0, width, height = self.state().get("viewport") or (0, 0, *self.size)
        self.xdo("mousemove", "--sync", "--window", self.window, round(x0 + width * 0.5), round(y0 + height - 4))
        time.sleep(0.8)
        self.capture_number += 1
        generated = self.f12 / f"screenshot-{self.capture_number:03}.png"
        generated.unlink(missing_ok=True)
        self.xdo("key", "F12")

        def complete():
            try:
                with Image.open(generated) as image:
                    image.load()
                    assert image.size == self.size, image.size
                return True
            except (OSError, ValueError):
                return False
        wait_for(complete, f"capture {name}", self.process)
        destination = self.output / f"{name}.png"
        generated.rename(destination)
        x, y, width, height = self.geometry()
        assert (width, height) == self.size
        presented = ImageGrab.grab(bbox=(x, y, x + width, y + height), xdisplay=self.env["DISPLAY"])
        presented.save(self.output / f"{name}-window.png")
        with Image.open(destination) as rendered:
            difference = sum(ImageStat.Stat(ImageChops.difference(rendered.convert("RGB"), presented.convert("RGB"))).mean) / 3
            viewport = self.state().get("viewport")
            if check_viewport and viewport:
                vx, vy, vw, vh = map(round, viewport)
                preview = rendered.convert("RGB").crop((vx, vy, vx + vw, vy + vh))
                colored = sum(1 for r, g, b in preview.get_flattened_data()
                              if ((max(r, g, b) > 70 and min(r, g, b) > 20) if neutral else
                                  (max(r, g, b) - min(r, g, b) > 40 and max(r, g, b) > 80)))
                assert colored > 1000, f"No visible asset in {name}"
        assert difference < 8, f"Rendered image differs from actual window: {difference}"
        (self.output / f"{name}-state.json").write_text(json.dumps(self.state(), indent=2) + "\n")
        self.report["captures"].append({"name": name, "window_mean_difference": difference})
        print(f"Captured {name} at {self.state()['revision']} ({len(self.report['actions'])} actions)", flush=True)


def selected_id(state, kind):
    match = re.fullmatch(kind + r"\((\d+)\)", state["selection"])
    assert match, state["selection"]
    return int(match[1])


def set_vector(s, name, values):
    for axis, value in enumerate(values):
        s.field(f"{name}({axis})", value)


def create_body(s):
    s.new_resource("Body")
    s.type_and_commit("Smoke goo person")
    assert s.library()["bodies"][0]["bones"] == []
    s.action("AddBone { child: false }")
    s.edit_text("BoneName", "Torso")
    torso = selected_id(s.state(), "Bone")
    set_vector(s, "BoneOffset", [0, 0.8, 0])
    set_vector(s, "BoneTip", [0, 0.7, 0])
    radius = s.library()["bodies"][0]["bones"][0]["radius"]
    s.scrub("BoneRadius", 40)
    assert s.library()["bodies"][0]["bones"][0]["radius"] > radius + 0.1, "Scrubbing did not change the radius"
    s.action("Undo")
    assert abs(s.library()["bodies"][0]["bones"][0]["radius"] - radius) < 1e-6, "One scrub is one undo step"
    s.field("BoneRadius", 0.27)
    s.action("AddBone { child: true }")
    s.edit_text("BoneName", "Head")
    set_vector(s, "BoneOffset", [0, 0.1, 0])
    set_vector(s, "BoneTip", [0, 0.3, 0])
    s.action("ToggleShape")
    s.field("BoneRadius", 0.33)
    # The viewport move handle edits the same values: drag the tip's Y arrow.
    before = s.library()
    s.drag_handle("BoneTip", "Axis(1)", 0, -50)
    head = s.library()["bodies"][0]["bones"][1]
    assert head["tip"][1] > 0.4 and abs(head["tip"][0]) < 1e-6, head["tip"]
    s.action("Undo")
    assert s.library() == before, "A handle drag is one undo step"
    s.action("AddMount")
    s.edit_text("MountName", "Hat socket")
    for name, offset, tip in [
        ("Left arm", [-0.2, -0.1, 0], [-0.5, -0.4, 0]),
        ("Right arm", [0.2, -0.1, 0], [0.5, -0.4, 0]),
        ("Left leg", [-0.2, -0.7, 0], [0, -0.7, 0]),
        ("Right leg", [0.2, -0.7, 0], [0, -0.7, 0]),
    ]:
        s.action(f"Select(Bone({torso}))")
        s.action("AddBone { child: true }")
        s.edit_text("BoneName", name)
        set_vector(s, "BoneOffset", offset)
        set_vector(s, "BoneTip", tip)
    body = s.library()["bodies"][0]
    assert len(body["bones"]) == 6 and len(body["mounts"]) == 1
    assert all(b["parent"] == body["bones"][0]["id"] for b in body["bones"][1:])
    assert body["bones"][1]["shape"].lower() == "sphere"
    assert [b["name"] for b in body["bones"]] == ["Torso", "Head", "Left arm", "Right arm", "Left leg", "Right leg"]
    assert body["mounts"][0]["name"] == "Hat socket"
    s.action("Save")
    assert not s.state()["dirty"]
    s.capture("body-from-scratch")


def paint(s, cursor):
    for axis, value in enumerate(cursor):
        if s.state()["cursor"][axis] != value:
            s.field(f"Cursor({axis})", value)
    s.action("PaintCursor { erase: false }")


def create_prop(s):
    s.new_resource("Prop")
    s.type_and_commit("Purple voxel hat")
    assert s.library()["props"][0]["blocks"] == []
    # A large cube is one stored block; a fine eraser preserves its surroundings.
    s.action("Brush(16)")
    for axis in range(3):
        s.field(f"Cursor({axis})", 0)
    s.action("PaintCursor { erase: false }")
    original = s.library()
    assert len(original["props"][0]["blocks"]) == 1
    s.action("Brush(1)")
    s.action("PaintCursor { erase: true }")
    erased = s.library()
    assert erased != original and len(erased["props"][0]["blocks"]) > 1
    s.action("Undo")
    assert s.library() == original
    s.action("Redo")
    assert s.library() == erased
    s.action("Brush(16)")
    s.action("PaintCursor { erase: true }")
    assert s.library()["props"][0]["blocks"] == []
    # Four-voxel cubes form a wide flat brim and a narrower raised crown.
    s.action("Brush(4)")
    s.action("Color(16)")
    for row, z in enumerate(range(0, 16, 4)):
        for x in (range(0, 16, 4) if row % 2 == 0 else range(12, -1, -4)):
            paint(s, [x, 0, z])
    s.action("Color(14)")
    for y in (4, 8):
        for x, z in ((4, 4), (8, 4), (8, 8), (4, 8)):
            paint(s, [x, y, z])
    for axis, value in enumerate([8, 0, 8]):
        s.field(f"Cursor({axis})", value)
    s.viewport_paint()
    s.action("AddAnchor")
    s.edit_text("AnchorName", "Head center")
    prop = s.library()["props"][0]
    assert len(prop["anchors"]) == 1
    assert all(abs(a - b) < 1e-6 for a, b in zip(prop["anchors"][0]["position"], [0.8, 0.0, 0.8]))
    assert sum(b["size"] ** 3 for b in prop["blocks"]) == 24 * 4 ** 3
    s.action("Save")
    s.capture("voxel-hat")


def create_character(s):
    s.new_resource("Character")
    s.type_and_commit("Goo person in hat")
    library = s.library()
    head = library["bodies"][0]["bones"][1]["id"]
    # Bones are selectable in the outline and drive the pose inspector.
    s.action(f"Select(Bone({head}))")
    assert s.state()["selection"] == f"Bone({head})"
    s.action("Select(None)")
    s.action("Attach")
    s.field("AttachScale", 0.5)
    library = s.library()
    character = library["characters"][0]
    assert character["body"] == library["bodies"][0]["id"]
    assert len(character["attachments"]) == 1
    attachment = character["attachments"][0]
    assert attachment["mount"] == library["bodies"][0]["mounts"][0]["id"]
    assert attachment["anchor"] == library["props"][0]["anchors"][0]["id"]
    assert attachment["prop"] == library["props"][0]["id"]
    assert abs(attachment["scale"] - 0.5) < 1e-6
    before_rotation = s.library()
    s.scrub("AttachRotation(1)", 30)
    assert s.library()["characters"][0]["attachments"][0]["rotation"][1] > 0.2
    s.action("Undo")
    assert s.library() == before_rotation
    s.action("ToggleGuides")
    s.action("Save")
    s.capture("character-hat")
    return s.library()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true")
    args = parser.parse_args()
    output = ROOT / "artifacts" / (time.strftime("creator-%Y%m%d-%H%M%S") + f"-{os.getpid()}")
    output.mkdir(parents=True)
    data = output / "data"
    print(f"Evidence: {output}", flush=True)
    report = {"status": "failed", "display": "Xvfb", "software": True, "resolution": SCREEN_SIZE,
              "actions": [], "captures": [], "scenario": "create six-bone body, voxel hat, attach, save and restart"}
    env = os.environ.copy()
    env.setdefault("CARGO_HOME", str(ROOT / ".cargo-cache"))
    xvfb = session = None
    try:
        if not args.no_build:
            with (output / "build.log").open("w") as log:
                subprocess.run(["cargo", "build", "--locked"], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        software_vulkan(env)
        xvfb = start_xvfb(env, SCREEN_SIZE, output / "xvfb.log")
        session = Session(env, output, data, report)
        empty = {"version": 1, "bodies": [], "props": [], "characters": [], "tiles": [], "worlds": [], "socket_rules": {"pairs": []}}
        assert session.state()["library"] == empty
        assert not session.state()["workspace_ready"]
        session.capture("welcome", check_viewport=False)
        session.action_starting("OpenWorkspace(")
        assert session.state()["workspace_ready"]
        session.capture("workspace", check_viewport=False)
        create_body(session)
        create_prop(session)
        expected = create_character(session)
        assert float32_model(json.loads((data / "library.json").read_text())) == float32_model(expected)
        stop(session.process)
        session = Session(env, output, data, report, workspace="characters", prefix="reopened")
        assert session.state()["mode"] == "Character"
        assert session.state()["library"] == expected, "Restart changed saved assets"
        session.action("ToggleGuides")
        session.capture("character-reopened")
        stop(session.process)
        with (output / "direct.log").open("w") as log:
            subprocess.run([str(game_binary()), "--mode", "editor", "--editor-view", "characters", "--data-dir", str(data),
                            "--capture", str(output / "character-direct.png"), "--capture-frame", "60", "--exit-after-capture"],
                           cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=120, check=True)
        for path in output.glob("*.png"):
            with Image.open(path) as image:
                assert image.size == SCREEN_SIZE, (path, image.size)
        for name in ("game.log", "reopened.log", "direct.log"):
            log = (output / name).read_text()
            errors = [line for line in log.splitlines() if (" ERROR " in line and "gilrs" not in line) or "panicked at" in line]
            assert not errors, f"Inspect {name}: {errors[-1]}"
            assert "capture_saved" in log, name
        with (output / "expected-capture-failure.log").open("w") as log:
            failure = subprocess.run([str(game_binary()), "--mode", "editor", "--editor-view", "characters", "--data-dir", str(data),
                                      "--capture", str(output / "invalid.unsupported"), "--capture-frame", "30", "--exit-after-capture"],
                                     cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=120)
        assert failure.returncode == 1, f"Capture failure exit code: {failure.returncode}"
        assert "capture_failed" in (output / "expected-capture-failure.log").read_text()
        assert float32_model(json.loads((data / "library.json").read_text())) == float32_model(expected)
        report.update(status="passed", persisted_body_bones=6, persisted_props=1, persisted_attachments=1,
                      capture_failure_exit_code=failure.returncode)
        print("PASS: body, sparse voxel edits, hat, attachment, real input, captures and save/restart.", flush=True)
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
        if session is not None:
            (output / "failure-state.json").write_text(json.dumps(session.state(), indent=2) + "\n")
        raise
    finally:
        if session is not None:
            stop(session.process)
        stop(xvfb)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"Report: {output / 'report.json'}", flush=True)


if __name__ == "__main__":
    main()
