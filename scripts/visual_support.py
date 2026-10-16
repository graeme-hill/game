"""Shared baseline support for real-window visual tests."""
from pathlib import Path
import os
import random
import select
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
SCREEN_SIZE = (1280, 720)


def game_binary():
    """The debug build, honouring CARGO_TARGET_DIR like `cargo build` does."""
    target = os.environ.get("CARGO_TARGET_DIR")
    return (Path(target) if target else ROOT / "target") / "debug" / "game"


def software_vulkan(env):
    """Point wgpu at Mesa's software Vulkan driver on a private X display."""
    drivers = [p for directory in env.get("XDG_DATA_DIRS", "/usr/share").split(":")
               for p in Path(directory).glob("vulkan/icd.d/lvp*.json")]
    if not drivers:
        raise RuntimeError("Mesa software Vulkan driver not found; run inside nix develop")
    for tool in ("xdotool", "Xvfb"):
        if not shutil.which(tool):
            raise RuntimeError(f"Missing {tool}; run inside nix develop")
    env.update(VK_DRIVER_FILES=str(drivers[-1]), VK_ICD_FILENAMES=str(drivers[-1]),
               WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1", WGPU_BACKEND="vulkan",
               RUST_LOG="info,wgpu_core=warn,wgpu_hal=warn", RUST_BACKTRACE="1",
               XDG_CACHE_HOME=str(ROOT / "artifacts" / "cache"))
    env.pop("WAYLAND_DISPLAY", None)
    return env


def start_xvfb(env, size, log_path):
    """Start a private Xvfb and set env["DISPLAY"].

    Set XVFB_TCP=1 where Unix sockets are unavailable (restricted sandboxes):
    the server then listens on localhost TCP instead. Xvfb still needs a
    writable /tmp for its lock file.
    """
    width, height = size
    log = open(log_path, "w")
    if os.environ.get("XVFB_TCP") == "1":
        number = random.randint(40, 99)
        process = subprocess.Popen(["Xvfb", f":{number}", "-screen", "0", f"{width}x{height}x24", "-listen", "tcp",
                                    "-nolisten", "unix", "-nolisten", "local", "-ac"],
                                   stdout=log, stderr=subprocess.STDOUT, env=env)
        env["DISPLAY"] = f"127.0.0.1:{number}"
        deadline = time.monotonic() + 15
        while subprocess.run(["xdotool", "getdisplaygeometry"], env=env, capture_output=True).returncode:
            if time.monotonic() > deadline or process.poll() is not None:
                raise RuntimeError("Xvfb did not start")
            time.sleep(0.2)
        return process
    read_fd, write_fd = os.pipe()
    process = subprocess.Popen(["Xvfb", "-displayfd", str(write_fd), "-screen", "0", f"{width}x{height}x24",
                                "-nolisten", "tcp", "-ac"], pass_fds=(write_fd,), stdout=log,
                               stderr=subprocess.STDOUT, env=env)
    os.close(write_fd)
    try:
        if not select.select([read_fd], [], [], 15)[0]:
            raise RuntimeError("Xvfb did not start")
        with os.fdopen(read_fd) as display_pipe:
            display = display_pipe.readline().strip()
    except Exception:
        stop(process)
        raise
    assert display.isdigit(), display
    env["DISPLAY"] = ":" + display
    return process


def wait_for(predicate, description, process=None, timeout=120):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        if process is not None and process.poll() is not None:
            raise RuntimeError(f"Game exited ({process.returncode}) while waiting for {description}")
        time.sleep(0.1)
    raise TimeoutError(f"Timed out waiting for {description}")


def stop(process):
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
