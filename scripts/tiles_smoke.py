#!/usr/bin/env python3
"""Real-input tile authoring, socket rules, graph placement, generation, persistence and curb/door traversal."""
import json
import math
import os
import shutil
import subprocess
import time
from PIL import Image
from creator_smoke import Session, float32_model
from visual_support import ROOT, game_binary, software_vulkan, start_xvfb, stop, wait_for


def logical(library):
    library = dict(library)
    for key in ("bodies", "props", "characters", "tiles", "worlds"):
        library[key] = sorted(library[key], key=lambda asset: asset["id"])
    return float32_model(library)


def resource_id(state, kind, name):
    key = {"Tile": "tiles", "World": "worlds"}[kind]
    return next(asset["id"] for asset in state["library"][key] if asset["name"] == name)


def open_resource(s, kind, name):
    """Find a resource with the Explorer filter and open it."""
    s.edit_text("Filter", name)
    doc = 0 if kind == "Rules" else resource_id(s.state(), kind, name)
    s.action(f"Open(DocRef {{ kind: {kind}, id: {doc} }})")
    assert s.state()["active"] == {"kind": kind, "id": doc}


def main():
    output = ROOT / "artifacts" / f"tiles-{time.strftime('%Y%m%d-%H%M%S')}-{os.getpid()}"
    output.mkdir(parents=True)
    data = output / "data"
    shutil.copytree(ROOT / "test_workspace", data)
    report = dict(status="failed", renderer="Mesa software Vulkan", actions=[], captures=[])
    env = software_vulkan(os.environ.copy())
    xvfb = s = game = None
    try:
        xvfb = start_xvfb(env, (1600, 900), output / "xvfb.log")
        s = Session(env, output, data, report, workspace="tiles", prefix="tiles")
        assert s.state()["mode"] == "Tile"

        def capture(name):
            s.capture(name, neutral=True)

        for name in ["Road straight", "Road corner", "Front door"]:
            open_resource(s, "Tile", name)
            capture(name.lower().replace(" ", "-"))
        # Author a new source asset through visible controls.
        s.new_resource("Tile")
        s.type_and_commit("Smoke tile")
        s.action("Brush(8)")
        s.action("PaintCursor { erase: false }")
        tile = next(t for t in s.library()["tiles"] if t["name"] == "Smoke tile")
        assert tile["voxels"], "Painting at the cursor added no voxels"
        s.action("AddSocket")
        assert s.state()["selection"] == "Socket(0)"
        s.action("ToggleRequired")
        s.field("SocketPosition(0)", 1)
        s.action("TurnSocket")
        # Socket types come from a dropdown; new ones are typed via its last entry.
        s.menu("OpenMenu(SocketKind)", "Edit(SocketKind)")
        s.wait(lambda v: v.get("editing") == "SocketKind", "custom socket type entry")
        s.type_and_commit("custom-path")
        s.edit_text("SocketProfile", "custom-width")
        tile = next(t for t in s.library()["tiles"] if t["name"] == "Smoke tile")
        socket = tile["sockets"][0]
        assert socket["required"] and socket["position"][0] == 1
        assert socket["kind"] == "custom-path" and socket["profile"] == "custom-width"
        s.action("AddCollider")
        assert next(t for t in s.library()["tiles"] if t["name"] == "Smoke tile")["colliders"]
        s.action("Undo")
        assert not next(t for t in s.library()["tiles"] if t["name"] == "Smoke tile")["colliders"]
        s.action("Redo")
        s.action("Frame")
        capture("tile-authoring")
        # The compatibility matrix is a document of its own.
        open_resource(s, "Rules", "Socket rules")
        s.action('ToggleRule("custom-path", "custom-path")')
        assert ["custom-path", "custom-path"] in s.library()["socket_rules"]["pairs"]
        s.capture("socket-rules", check_viewport=False)

        open_resource(s, "World", "Neighbourhood")
        original_id = s.state()["active"]["id"]
        s.action("Validate")
        assert s.state()["notice_kind"] == "Success", s.state()["notice"]
        capture("neighbourhood")
        # Manual placement: a first tile, then a tile chosen at a clicked socket.
        s.new_resource("World")
        s.type_and_commit("Manual roads")
        straight = resource_id(s.state(), "Tile", "Road straight")
        corner = resource_id(s.state(), "Tile", "Road corner")
        s.menu("OpenMenu(PlaceTile)", f"SetPlaceTile({straight})")
        s.action("Place")
        world = s.state()["active"]["id"]
        s.action("Frame")
        state = s.wait(lambda v: any(c["kind"] == "road" for c in v.get("sockets", [])), "open road socket on screen")
        road = next(c for c in state["sockets"] if c["kind"] == "road")
        s.press({"center": road["center"], "size": [1, 1]}, "click open road socket")
        s.wait(lambda v: v.get("menu") == "Connect", "connect menu at the socket")
        state = s.settle()
        option = next(c for c in state["controls"]
                      if c["kind"] == "MenuItem" and c["action"].startswith(f"PlaceOption {{ tile: {corner},"))
        s.press(option, option["action"])
        w = next(v for v in s.library()["worlds"] if v["id"] == world)
        assert len(w["instances"]) == 2 and len(w["connections"]) == 1, w
        s.action("Frame")
        capture("manual-road-corner")
        s.action("RemovePiece")
        assert len(next(v for v in s.library()["worlds"] if v["id"] == world)["instances"]) == 1
        s.action("Undo")
        assert len(next(v for v in s.library()["worlds"] if v["id"] == world)["instances"]) == 2
        s.field("Seed", 8)
        s.action("Generate")
        assert s.state()["active"]["kind"] == "World" and s.state()["active"]["id"] != world
        s.action("Validate")
        assert s.state()["notice_kind"] == "Success", s.state()["notice"]
        s.action("Save")
        expected = s.library()
        capture("generated-world")
        s.menu("OpenMenu(File)", "Revert")
        assert logical(s.library()) == logical(expected)
        stop(s.process)
        s = None
        s = Session(env, output, data, report, workspace="worlds", prefix="reopened")
        assert logical(s.state()["library"]) == logical(expected)
        stop(s.process)
        s = None
        state_path = output / "play-state.json"
        command = [str(game_binary()), "--mode", "game", "--workspace", str(data), "--world", str(original_id),
                   "--state-file", str(state_path), "--output-dir", str(output / "play"),
                   "--capture", str(output / "play-ready.png"), "--capture-frame", "60"]
        with (output / "play.log").open("w") as log:
            game = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)

        def state():
            try:
                return json.loads(state_path.read_text())
            except (OSError, ValueError):
                return {}

        def wait(pred, label):
            wait_for(lambda: pred(state()), label, game, timeout=120)
            return state()
        wait(lambda v: v.get("world") and v.get("collision_boxes", 0) > 0, "world startup")
        wait_for(lambda: "capture_saved" in (output / "play.log").read_text(), "renderer warmup", game, timeout=120)

        def xdo(*args):
            return subprocess.check_output(["xdotool", *map(str, args)], env=env, text=True, timeout=10).strip()
        window = xdo("search", "--sync", "--onlyvisible", "--pid", game.pid).splitlines()[0]
        xdo("windowfocus", "--sync", window)
        xdo("mousemove", "--window", window, 640, 360)
        xdo("click", 1)
        wait(lambda v: v.get("captured"), "capture play controls")
        start = state()["position"]
        yaw = state()["camera_yaw"]
        forward = (-math.sin(yaw), -math.cos(yaw))

        def distance(v):
            return (v["position"][0] - start[0]) * forward[0] + (v["position"][2] - start[2]) * forward[1]
        xdo("keydown", "w")
        curb = wait(lambda v: distance(v) > 1. and v["position"][1] > .18, "step from road onto sidewalk")
        door = wait(lambda v: distance(v) > 10.7, "walk along path and through front door")
        xdo("keyup", "w")
        wait(lambda v: v["speed"] < .01, "stop inside house")
        assert abs(door["position"][1] - .2) < .03
        xdo("key", "F12")
        capture_path = output / "play/screenshot-001.png"
        wait_for(lambda: capture_path.exists(), "house capture", game)
        time.sleep(1)
        with Image.open(capture_path) as im:
            assert im.size == (1280, 720)
        shutil.copy(capture_path, output / "inside-house.png")
        xdo("key", "space")
        wait(lambda v: not v["grounded"], "jump inside house")
        wait(lambda v: v["grounded"], "land on house floor")
        # Continue into the back wall and verify collision stops movement. Hold
        # the key until the wall is reached (software rendering can be slow),
        # then keep pushing to show the wall holds.
        xdo("keydown", "w")
        try:
            wait_for(lambda: distance(state()) > 13.9, "reach the back wall", game, timeout=30)
        except TimeoutError:
            pass
        time.sleep(2)
        xdo("keyup", "w")
        wait(lambda v: v["speed"] < .01, "wall stop")
        wall = state()
        assert 14 < distance(wall) < 16.4, (distance(wall), wall["position"])
        for name in ["tiles.log", "reopened.log", "play.log"]:
            log = (output / name).read_text()
            errors = [line for line in log.splitlines() if (" ERROR " in line and "gilrs" not in line) or "panicked at" in line]
            assert not errors, f"{name}: {errors[-1]}"
        report.update(status="passed", game_command=command,
                      gameplay={"curb": curb["position"], "door": door["position"], "wall": wall["position"]})
        print("PASS: voxel/socket/rules/collision editing, socket-click placement, seeded generation, restart, curb, doorway and wall collision.", flush=True)
    except Exception as error:
        report["error"] = repr(error)
        if s is not None:
            (output / "failure-state.json").write_text(json.dumps(s.state(), indent=2) + "\n")
        raise
    finally:
        if s:
            stop(s.process)
        stop(game)
        stop(xvfb)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print("Report:", output / "report.json", flush=True)


if __name__ == "__main__":
    main()
