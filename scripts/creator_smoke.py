#!/usr/bin/env python3
"""Create a rest-pose character and voxel hat through the 1280×720 UI."""
import argparse
import json
import os
from pathlib import Path
import re
import select
import struct
import shutil
import subprocess
import time

from PIL import Image, ImageChops, ImageGrab, ImageStat
from visual_support import ROOT, SCREEN_SIZE, stop, wait_for


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
    def __init__(self, env, output, data, report, workspace=None, prefix="game"):
        self.env, self.output, self.report = env, output, report
        self.state_path = output / "state.json"
        self.state_path.unlink(missing_ok=True)
        self.capture_number = 0
        self.prefix = prefix
        ready_capture = output / f"{prefix}-ready.png"
        command = [str(ROOT / "target/debug/game"), "--mode", "editor", "--data-dir", str(data),
                   "--state-file", str(self.state_path), "--output-dir", str(output),
                   "--capture", str(ready_capture), "--capture-frame", "120"]
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
            geometry = dict(line.split("=", 1) for line in self.xdo("getwindowgeometry", "--shell", self.window).splitlines())
            assert (int(geometry["WIDTH"]), int(geometry["HEIGHT"])) == SCREEN_SIZE, geometry
        except Exception:
            stop(self.process)
            raise

    def xdo(self, *arguments):
        return subprocess.check_output(["xdotool", *map(str, arguments)], env=self.env, text=True, timeout=10).strip()

    def state(self):
        try:
            return json.loads(self.state_path.read_text())
        except (OSError, ValueError):
            return {}

    def wait(self, predicate, description):
        wait_for(lambda: predicate(self.state()), description, self.process, timeout=120)
        return self.state()

    def action(self, action):
        before = self.wait(lambda s: bool(s.get("controls")), "controls before " + action)
        controls = [c for c in before["controls"] if c["action"] == action]
        assert controls, f"Missing {action}: available {[c['action'] for c in before['controls']]}"
        control = controls[0]
        x, y = control["center"]
        width, height = control["size"]
        assert 0 <= x < SCREEN_SIZE[0] and 0 <= y < SCREEN_SIZE[1] and width > 0 and height > 0, control
        revision = before["revision"]
        self.xdo("mousemove", "--sync", "--window", self.window, round(x), round(y))
        time.sleep(0.07)
        self.xdo("mousedown", 1)
        time.sleep(0.2)
        self.xdo("mouseup", 1)
        after = self.wait(lambda s: s.get("revision", 0) > revision, action)
        # Bevy rebuilds controls after edits. Let pointer release/layout propagate.
        time.sleep(0.08)
        self.report["actions"].append({"action": action, "revision": after["revision"], "notice": after.get("notice")})
        if len(self.report["actions"]) % 20 == 0:
            print(f"Progress: {len(self.report['actions'])} UI actions; {action}; revision {after['revision']}", flush=True)
        if any(word in after.get("notice", "").lower() for word in ("rejected", "failed", "cannot", "maximum")):
            raise AssertionError(after["notice"])
        errors = [line for line in self.log_path.read_text().splitlines() if " ERROR " in line or "panicked at" in line]
        if errors:
            raise AssertionError(f"Renderer error after {action}: {errors[-1]}")
        return after

    def signed_action(self, name, positive):
        for control in self.state()["controls"]:
            match = re.fullmatch(re.escape(name) + r"\((-?[0-9.]+)\)", control["action"])
            if match and float(match[1]) != 0 and (float(match[1]) > 0) == positive:
                return control["action"], abs(float(match[1]))
        raise AssertionError(f"No {name} increment, positive={positive}")

    def new_resource(self, kind):
        action = f"NewResource({kind})"
        if not any(control["action"] == action for control in self.state()["controls"]):
            self.action("ToggleNewMenu")
        self.action(action)

    def rename(self, target, name):
        self.action(f"Rename({target})")
        self.xdo("keydown", "Control_L")
        time.sleep(0.15)
        self.xdo("key", "a")
        time.sleep(0.15)
        self.xdo("keyup", "Control_L")
        self.wait(lambda s: s.get("naming") == "", "name cleared")
        self.xdo("type", "--clearmodifiers", "--delay", 65, name)
        self.xdo("key", "Return")
        self.wait(lambda s: s.get("naming") is None, "name committed")
        state = self.state()
        assets = state["library"][state["mode"].lower()]
        asset = assets[state[{"Bodies": "body", "Props": "prop", "Characters": "character"}[state["mode"]]]]
        item = asset if target == "Asset" else asset[target.lower() + "s"][state[target.lower()]]
        assert item["name"] == name, (item["name"], name)
        time.sleep(0.1)

    def viewport_paint(self):
        before = self.state()["library"]
        width, height = SCREEN_SIZE
        # Probe the same relative preview locations at every baseline size;
        # the preview expands with the live window rather than staying at
        # the old 640x480 pixel coordinates.
        for fx, fy in ((0.45, 0.48), (0.40, 0.50), (0.50, 0.50), (0.40, 0.40),
                       (0.55, 0.60), (0.65, 0.60), (0.75, 0.60), (0.85, 0.60),
                       (0.60, 0.70), (0.70, 0.70), (0.80, 0.70), (0.90, 0.70),
                       (0.65, 0.80), (0.75, 0.80), (0.85, 0.80)):
            x, y = round(width * fx), round(height * fy)
            self.xdo("mousemove", "--sync", "--window", self.window, x, y)
            time.sleep(0.1)
            self.xdo("mousedown", 1)
            time.sleep(0.2)
            self.xdo("mouseup", 1)
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                if self.state().get("library") != before:
                    self.report["viewport_paint"] = {"position": [x, y], "revision": self.state()["revision"]}
                    self.capture("viewport-painted")
                    self.action("Undo")
                    assert self.state()["library"] == before
                    return
                time.sleep(0.1)
        raise AssertionError("No direct viewport click painted a voxel")

    def vector(self, field, current, target):
        self.action(f"SetVector({field})")
        for axis in range(3):
            steps = round((target[axis] - current[axis]) / 0.1)
            for _ in range(abs(steps)):
                self.action(f"Axis({axis}, {'0.1' if steps > 0 else '-0.1'})")

    def brush(self, size):
        while self.state()["brush"] != size:
            self.action("Brush(1)")

    def cursor(self, target):
        for axis in range(3):
            while self.state()["cursor"][axis] != target[axis]:
                current = self.state()["cursor"][axis]
                step = self.state()["brush"]
                assert abs(target[axis] - current) >= step, (target, self.state()["cursor"], step)
                self.action(f"Cursor({axis}, {1 if target[axis] > current else -1})")

    def capture(self, name, neutral=False):
        self.xdo("mousemove", "--sync", "--window", self.window, 638, 478)
        time.sleep(0.8)
        self.capture_number += 1
        generated = self.output / f"screenshot-{self.capture_number:03}.png"
        generated.unlink(missing_ok=True)
        self.xdo("key", "F12")
        def complete():
            try:
                with Image.open(generated) as image:
                    image.load()
                    assert image.size == SCREEN_SIZE, image.size
                return True
            except (OSError, ValueError):
                return False
        wait_for(complete, f"capture {name}", self.process)
        destination = self.output / f"{name}.png"
        generated.rename(destination)
        geometry = dict(line.split("=", 1) for line in self.xdo("getwindowgeometry", "--shell", self.window).splitlines())
        x, y, width, height = (int(geometry[key]) for key in ("X", "Y", "WIDTH", "HEIGHT"))
        assert (width, height) == SCREEN_SIZE
        presented = ImageGrab.grab(bbox=(x, y, x + width, y + height), xdisplay=self.env["DISPLAY"])
        presented.save(self.output / f"{name}-window.png")
        with Image.open(destination) as rendered:
            difference = sum(ImageStat.Stat(ImageChops.difference(rendered.convert("RGB"), presented.convert("RGB"))).mean) / 3
            if name != "menu":
                preview = rendered.convert("RGB").crop((235, 70, 990, 660))
                colored = sum(1 for r, g, b in preview.get_flattened_data()
                              if ((max(r, g, b) > 70 and min(r, g, b) > 20) if neutral else
                                  (max(r, g, b) - min(r, g, b) > 40 and max(r, g, b) > 80)))
                assert colored > 1000, f"No visible asset in {name}"
        assert difference < 8, f"Rendered image differs from actual window: {difference}"
        (self.output / f"{name}-state.json").write_text(json.dumps(self.state(), indent=2) + "\n")
        self.report["captures"].append({"name": name, "window_mean_difference": difference})
        print(f"Captured {name} at {self.state()['revision']} ({len(self.report['actions'])} actions)", flush=True)


def create_body(s):
    s.new_resource("Bodies")
    assert s.state()["library"]["bodies"][0]["bones"] == []
    s.rename("Asset", "Smoke goo person")
    s.action("AddBone(false)")
    s.rename("Bone", "Torso")
    s.vector("BoneOffset", [0, 0, 0], [0, 0.8, 0])
    s.vector("BoneTip", [0, 0.6, 0], [0, 0.7, 0])
    action, _ = s.signed_action("Radius", True)
    for _ in range(3):
        s.action(action)
    s.action("AddBone(true)")
    s.rename("Bone", "Head")
    s.vector("BoneOffset", [0, 0, 0], [0, 0.1, 0])
    s.vector("BoneTip", [0, 0.6, 0], [0, 0.3, 0])
    s.action("Shape")
    for _ in range(5):
        s.action(action)
    s.action("BodyTools(true)")
    s.action("AddMount")
    s.rename("Mount", "Hat socket")
    s.action("BodyTools(false)")
    for name, offset, tip in [
        ("Left arm", [-0.2, -0.1, 0], [-0.5, -0.4, 0]),
        ("Right arm", [0.2, -0.1, 0], [0.5, -0.4, 0]),
        ("Left leg", [-0.2, -0.7, 0], [0, -0.7, 0]),
        ("Right leg", [0.2, -0.7, 0], [0, -0.7, 0]),
    ]:
        while s.state()["bone"] != 0:
            s.action("BoneCycle(-1)")
        s.action("AddBone(true)")
        s.rename("Bone", name)
        s.vector("BoneOffset", [0, 0, 0], offset)
        s.vector("BoneTip", [0, 0.6, 0], tip)
    body = s.state()["library"]["bodies"][0]
    assert len(body["bones"]) == 6 and len(body["mounts"]) == 1
    assert all(b["parent"] == body["bones"][0]["id"] for b in body["bones"][1:])
    assert body["bones"][1]["shape"].lower() == "sphere"
    assert [b["name"] for b in body["bones"]] == ["Torso", "Head", "Left arm", "Right arm", "Left leg", "Right leg"]
    s.action("Save")
    s.capture("body-from-scratch")


def create_prop(s):
    s.new_resource("Props")
    s.rename("Asset", "Purple voxel hat")
    assert s.state()["library"]["props"][0]["blocks"] == []
    # A large cube is one stored block; a fine eraser preserves its surroundings.
    s.cursor([0, 0, 0])
    s.brush(16)
    s.action("Paint(false)")
    original = s.state()["library"]
    assert len(original["props"][0]["blocks"]) == 1
    s.brush(1)
    s.action("Paint(true)")
    erased = s.state()["library"]
    assert erased != original and len(erased["props"][0]["blocks"]) > 1
    s.action("Undo")
    assert s.state()["library"] == original
    s.action("Redo")
    assert s.state()["library"] == erased
    s.brush(16)
    s.action("Paint(true)")
    assert s.state()["library"]["props"][0]["blocks"] == []
    # Four-voxel cubes form a wide flat brim and a narrower raised crown.
    s.brush(4)
    s.action("Color(7)")
    for row, z in enumerate(range(0, 16, 4)):
        for x in (range(0, 16, 4) if row % 2 == 0 else range(12, -1, -4)):
            s.cursor([x, 0, z])
            s.action("Paint(false)")
    s.action("Color(2)")
    for y in (4, 8):
        for x, z in ((4, 4), (8, 4), (8, 8), (4, 8)):
            s.cursor([x, y, z])
            s.action("Paint(false)")
    s.cursor([8, 0, 8])
    s.viewport_paint()
    s.cursor([8, 0, 8])
    s.action("PropTools(true)")
    s.action("AddAnchor")
    s.rename("Anchor", "Head center")
    prop = s.state()["library"]["props"][0]
    assert len(prop["anchors"]) == 1
    assert all(abs(a - b) < 1e-6 for a, b in zip(prop["anchors"][0]["position"], [0.8, 0.0, 0.8]))
    assert sum(b["size"] ** 3 for b in prop["blocks"]) == 24 * 4 ** 3
    s.action("Save")
    s.capture("voxel-hat")


def create_character(s):
    s.new_resource("Characters")
    s.rename("Asset", "Goo person in hat")
    # Exercise the child-friendly named bone tree in the character workspace.
    s.action("SelectBone(1)")
    assert s.state()["bone"] == 1
    s.action("Attach")
    action, step = s.signed_action("Scale", False)
    for _ in range(round(0.5 / step)):
        s.action(action)
    library = s.state()["library"]
    character = library["characters"][0]
    assert character["body"] == library["bodies"][0]["id"]
    assert len(character["attachments"]) == 1
    attachment = character["attachments"][0]
    assert attachment["mount"] == library["bodies"][0]["mounts"][0]["id"]
    assert attachment["anchor"] == library["props"][0]["anchors"][0]["id"]
    assert attachment["prop"] == library["props"][0]["id"]
    before_rotation = s.state()["library"]
    s.action("SetVector(AttachmentRotation)")
    rotation_action = next(c["action"] for c in s.state()["controls"] if c["action"].startswith("Axis(1, ") and "-" not in c["action"])
    s.action(rotation_action)
    assert s.state()["library"] != before_rotation
    assert s.state()["library"]["characters"][0]["attachments"][0]["rotation"][1] > 0.2
    s.action("Undo")
    assert s.state()["library"] == before_rotation
    s.action("SetVector(AttachmentOffset)")
    s.action("Guides")
    s.action("Save")
    s.capture("character-hat")
    return s.state()["library"]


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
    env["XDG_CACHE_HOME"] = str(ROOT / "artifacts" / "cache")
    xvfb = session = None
    try:
        if not args.no_build:
            with (output / "build.log").open("w") as log:
                subprocess.run(["cargo", "build", "--locked"], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        for tool in ("xdotool", "Xvfb"):
            if not shutil.which(tool):
                raise RuntimeError(f"Missing {tool}; run inside nix develop")
        drivers = [p for directory in env.get("XDG_DATA_DIRS", "/usr/share").split(":") for p in Path(directory).glob("vulkan/icd.d/lvp*.json")]
        if not drivers:
            raise RuntimeError("Mesa software Vulkan driver not found")
        env.update(VK_DRIVER_FILES=str(drivers[-1]), VK_ICD_FILENAMES=str(drivers[-1]),
                   WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1", WGPU_BACKEND="vulkan",
                   RUST_LOG="info,wgpu_core=warn,wgpu_hal=warn", RUST_BACKTRACE="1")
        env.pop("WAYLAND_DISPLAY", None)
        read_fd, write_fd = os.pipe()
        with (output / "xvfb.log").open("w") as log:
            xvfb = subprocess.Popen(["Xvfb", "-displayfd", str(write_fd), "-screen", "0", "1280x720x24", "-nolisten", "tcp", "-ac"],
                                    pass_fds=(write_fd,), stdout=log, stderr=subprocess.STDOUT, env=env)
        os.close(write_fd)
        try:
            if not select.select([read_fd], [], [], 15)[0]:
                raise RuntimeError("Xvfb did not start")
            display_bytes = bytearray()
            while not display_bytes.endswith(b"\n"):
                part = os.read(read_fd, 1)
                if not part: raise RuntimeError("Xvfb closed display pipe")
                display_bytes.extend(part)
            display = display_bytes.decode().strip()
            assert display.isdigit(), display
            env["DISPLAY"] = ":" + display
        finally:
            os.close(read_fd)
        session = Session(env, output, data, report)
        assert session.state()["library"] == {"version": 1, "bodies": [], "props": [], "characters": [], "tiles": [], "worlds": [], "socket_rules": {"pairs": []}}
        session.action("ShowWorkspacePicker")
        session.action(next(control["action"] for control in session.state()["controls"]
                            if control["action"].startswith("SelectWorkspace(")))
        session.capture("menu")
        create_body(session)
        create_prop(session)
        expected = create_character(session)
        assert float32_model(json.loads((data / "library.json").read_text())) == float32_model(expected)
        stop(session.process)
        session = Session(env, output, data, report, workspace="characters", prefix="reopened")
        assert session.state()["mode"] == "Characters"
        assert session.state()["library"] == expected, "Restart changed saved assets"
        session.action("Guides")
        session.capture("character-reopened")
        stop(session.process)
        with (output / "direct.log").open("w") as log:
            subprocess.run([str(ROOT / "target/debug/game"), "--mode", "editor", "--editor-view", "characters", "--data-dir", str(data),
                            "--capture", str(output / "character-direct.png"), "--capture-frame", "60", "--exit-after-capture"],
                           cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=120, check=True)
        for path in output.glob("*.png"):
            with Image.open(path) as image:
                assert image.size == SCREEN_SIZE, (path, image.size)
        for name in ("game.log", "reopened.log", "direct.log"):
            log = (output / name).read_text()
            assert "ERROR" not in log and "panicked at" not in log, f"Inspect {name}"
            assert "capture_saved" in log, name
        with (output / "expected-capture-failure.log").open("w") as log:
            failure = subprocess.run([str(ROOT / "target/debug/game"), "--mode", "editor", "--editor-view", "characters", "--data-dir", str(data),
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
