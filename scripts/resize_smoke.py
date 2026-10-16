#!/usr/bin/env python3
"""Verify runtime resizing, responsive layout, and captures at several sizes."""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import time

from PIL import Image, ImageGrab

from visual_support import ROOT, game_binary, software_vulkan, start_xvfb, stop, wait_for

SIZES = [(1280, 720), (1600, 900), (800, 600), (640, 480), (1280, 720)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["editor", "game"], default="editor")
    parser.add_argument("--editor-view", choices=["bodies", "props", "characters", "tiles", "worlds"])
    parser.add_argument("--default-launch", action="store_true", help="Verify game/workspace defaults in an isolated directory")
    args = parser.parse_args()
    if args.mode == "game" and args.editor_view:
        parser.error("--editor-view requires --mode editor")
    if args.default_launch and args.mode != "game":
        parser.error("--default-launch requires --mode game")
    output = ROOT / "artifacts" / f"resize-{args.mode}-{time.strftime('%Y%m%d-%H%M%S')}-{os.getpid()}"
    output.mkdir(parents=True)
    data = output / ("test_workspace" if args.default_launch else "data")
    if args.mode == "game" or args.editor_view:
        shutil.copytree(ROOT / "test_workspace", data)

    def workspace_digest():
        return {str(path.relative_to(data)): hashlib.sha256(path.read_bytes()).hexdigest()
                for path in data.rglob("*") if path.is_file()}

    original = workspace_digest()

    def capture_complete(path):
        try:
            with Image.open(path) as captured:
                captured.load()
            return True
        except (OSError, ValueError):
            return False
    state_path = output / "state.json"
    env = software_vulkan(os.environ.copy())
    env.setdefault("CARGO_HOME", str(ROOT / ".cargo-cache"))
    xvfb = game = None
    report = {"status": "failed", "mode": args.mode, "default_launch": args.default_launch,
              "sizes": SIZES, "renderer": "Mesa software Vulkan"}
    try:
        xvfb = start_xvfb(env, (1600, 900), output / "xvfb.log")
        if args.mode == "game":
            # These failures must be handled before any window opens.
            for name, arguments, code, message in [
                ("missing-argument", ["--workspace"], 2, "requires a value"),
                ("missing-directory", ["--mode", "game", "--workspace", str(output / "absent")], 1, "not a directory"),
                ("empty-workspace", ["--mode", "game", "--workspace", str(output / "empty")], 1, "No characters"),
            ]:
                (output / "empty").mkdir(exist_ok=True)
                result = subprocess.run([str(game_binary()), *arguments], cwd=ROOT, env=env,
                                        text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=15)
                (output / f"{name}.log").write_text(result.stdout)
                assert result.returncode == code and message in result.stdout, result.stdout
        command = [str(game_binary()), "--state-file", str(state_path), "--output-dir", str(output)]
        if not args.default_launch:
            command += ["--mode", "game", "--workspace", str(data)] if args.mode == "game" else ["--mode", "editor", "--data-dir", str(data)]
        if args.editor_view:
            command += ["--mode", "editor", "--workspace", str(data), "--editor-view", args.editor_view]
        ready_capture = output / f"{args.mode}-ready.png"
        command += ["--capture", str(ready_capture), "--capture-frame", "60"]
        with (output / "game.log").open("w") as log:
            game = subprocess.Popen(command,
                                    cwd=output if args.default_launch else ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)

        def state():
            try:
                return json.loads(state_path.read_text())
            except (OSError, ValueError):
                return {}

        if args.mode == "game":
            wait_for(lambda: state().get("spawned_parts") == 3, "spawned character", game)
            assert state()["mode"] == "Game" and state()["character"]["name"] == "Stickman", state()
        else:
            wait_for(lambda: bool(state().get("controls")), "initial controls", game)
        wait_for(lambda: capture_complete(ready_capture), "render warmup", game)
        window = subprocess.check_output(["xdotool", "search", "--sync", "--onlyvisible", "--pid", str(game.pid)], env=env, text=True).splitlines()[0]
        subprocess.run(["xdotool", "windowfocus", "--sync", window], env=env, check=True)
        if args.mode == "game":
            subprocess.run(["xdotool", "mousemove", "--window", window, "640", "360", "click", "1"], env=env, check=True)
            wait_for(lambda: state().get("captured"), "active gameplay HUD", game)
        captures = []
        for index, (width, height) in enumerate(SIZES, 1):
            subprocess.run(["xdotool", "windowsize", "--sync", window, str(width), str(height)], env=env, check=True)
            wait_for(lambda: state().get("window") == [width, height], f"state resize {width}x{height}", game)
            current = state()
            if args.mode == "game":
                assert len(current["mounted_props"]) == 2, "Equipment missing after resize"
            geometry = dict(line.split("=", 1) for line in subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window], env=env, text=True).splitlines())
            assert [int(geometry["WIDTH"]), int(geometry["HEIGHT"])] == [width, height], geometry
            for control in current["controls"]:
                x, y = control["center"]
                assert 0 <= x < width and 0 <= y < height and control["size"][0] > 0 and control["size"][1] > 0, (width, height, control)
            subprocess.run(["xdotool", "key", "F12"], env=env, check=True)
            path = output / f"screenshot-{index:03}.png"
            wait_for(lambda: capture_complete(path), str(path), game)
            with Image.open(path) as image:
                image.load()
                assert image.size == (width, height), (path, image.size, (width, height))
                if args.mode == "game":
                    pixels = image.convert("RGB")
                    body = [(x, y) for y in range(height) for x in range(width)
                            if (lambda r, g, b: g > r * 1.5 and g > b * 1.03 and b > r * 1.5 and g > 55)(*pixels.getpixel((x, y))) ]
                    assert len(body) > width * height * 0.008, "Character surface missing"
                    assert min(x for x, _ in body) > 0 and max(x for x, _ in body) < width - 1
                    assert min(y for _, y in body) > 0 and max(y for _, y in body) < height - 1
                else:
                    assert sum(1 for r, g, b in image.convert("RGB").get_flattened_data()
                               if min(r, g, b) > 150) > 100, "Editor text missing"
            x, y = int(geometry["X"]), int(geometry["Y"])
            window_capture = output / f"window-{index:03}.png"
            ImageGrab.grab(bbox=(x, y, x + width, y + height), xdisplay=env["DISPLAY"]).save(window_capture)
            captures.append({"size": [width, height], "renderer": path.name, "window": window_capture.name})
        log_text = (output / "game.log").read_text()
        errors = [line for line in log_text.splitlines() if (" ERROR " in line and "gilrs" not in line) or "panicked at" in line]
        assert not errors, f"Inspect game.log: {errors[-1]}"
        if args.mode == "game":
            assert original == workspace_digest(), "Play mode changed the workspace"
            assert "game_character_spawned" in log_text
        report.update(status="passed", captures=captures, command=command)
        print(f"PASS: resized one live window through {SIZES}; every control stayed in bounds and every capture matched its size.", flush=True)
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        stop(game)
        stop(xvfb)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"Report: {output / 'report.json'}", flush=True)


if __name__ == "__main__":
    main()
