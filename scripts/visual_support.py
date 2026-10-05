"""Shared baseline support for real-window visual tests."""
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
SCREEN_SIZE = (1280, 720)


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
