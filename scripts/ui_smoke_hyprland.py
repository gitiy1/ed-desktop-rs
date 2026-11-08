#!/usr/bin/env python3
"""Exercise a native GPUI demo window on Hyprland with its Lua dispatcher API.

Requires hyprctl, grim and an already-built client. Captures only this process's
window and restores the pointer/focus after closing it. All app data is synthetic.
GPUI's Rust tests assert state changes; these captures check the real renderer.
"""

import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def run(*args):
    result = subprocess.run(args, capture_output=True, text=True, check=True)
    if result.stdout.lstrip().startswith("error:"):
        raise RuntimeError(result.stdout)
    return result.stdout


def dispatch(expression):
    run("hyprctl", "dispatch", expression)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/endao-desktop"))
    parser.add_argument("--output", type=Path, default=Path("/tmp/endao-ui-review"))
    parser.add_argument("--check-system-theme", action="store_true",
                        help="Temporarily switch the GNOME color-scheme setting and restore it afterward")
    args = parser.parse_args()
    if not os.environ.get("WAYLAND_DISPLAY"):
        parser.error("Run from a Hyprland Wayland session")
    args.output.mkdir(parents=True, exist_ok=True)
    old_active = json.loads(run("hyprctl", "-j", "activewindow"))
    old_pointer = json.loads(run("hyprctl", "-j", "cursorpos"))
    old_color_scheme = run("gsettings", "get", "org.gnome.desktop.interface", "color-scheme").strip() if args.check_system_theme else None
    with (args.output / "native-demo.log").open("w") as log:
        app = subprocess.Popen([str(args.binary.resolve()), "--demo", "library"], stdout=log, stderr=log)
        try:
            window = None
            for _ in range(60):
                if app.poll() is not None:
                    raise RuntimeError("Client exited; see native-demo.log")
                matches = [item for item in json.loads(run("hyprctl", "-j", "clients")) if item["pid"] == app.pid]
                if matches:
                    window = matches[0]
                    break
                time.sleep(0.25)
            if window is None:
                raise RuntimeError("GPUI window was not mapped")
            selector = json.dumps("address:" + window["address"])
            dispatch(f'hl.dsp.window.float({{action="enable",window={selector}}})')
            dispatch(f"hl.dsp.window.resize({{x=1180,y=820,relative=false,window={selector}}})")
            dispatch(f"hl.dsp.window.center({{window={selector}}})")
            time.sleep(0.5)

            def current():
                return next(item for item in json.loads(run("hyprctl", "-j", "clients")) if item["pid"] == app.pid)

            def click(x, y):
                item = current()
                at_x, at_y = item["at"]
                dispatch(f"hl.dsp.cursor.move({{x={at_x + x},y={at_y + y}}})")
                dispatch(f'hl.dsp.send_shortcut({{mods="",key="mouse:272",window={selector}}})')
                time.sleep(0.8)

            def capture(name):
                item = current()
                x, y = item["at"]
                width, height = item["size"]
                run("grim", "-g", f"{x},{y} {width}x{height}", str(args.output / f"{name}.png"))
                print(f"Captured {name}: {width} x {height}", flush=True)

            def background_pixel():
                item = current()
                x, y = item["at"]
                # Sample the main page gutter, outside cards, controls, and borders.
                ppm = subprocess.check_output(["grim", "-s", "1", "-t", "ppm", "-g", f"{x + 220},{y + 85} 1x1", "-"])
                header, dimensions, maximum, pixels = ppm.split(b"\n", 3)
                if (header, dimensions, maximum) != (b"P6", b"1 1", b"255"):
                    raise RuntimeError("Unexpected screenshot pixel format")
                return tuple(pixels[:3])

            def expect_background(expected):
                for _ in range(25):
                    actual = background_pixel()
                    if all(abs(a - b) <= 3 for a, b in zip(actual, expected)):
                        return
                    time.sleep(0.12)
                raise AssertionError(f"Expected page color {expected}, got {actual}")

            dark = (0x12, 0x1b, 0x18)
            light = (0xf3, 0xf6, 0xf4)

            capture("library")
            click(292, 196)  # Select all books.
            capture("selected")
            click(1064, 196)  # Start the synthetic queue.
            capture("downloads")
            click(940, 129)  # Clear the synthetic completed entry.
            capture("completed-cleared")
            click(1080, 205)  # Pause just the first task; the next one starts.
            capture("task-paused")
            click(1080, 205)  # Queue the first task for resume.
            capture("task-resumed")
            dispatch(f"hl.dsp.window.resize({{x=960,y=640,relative=false,window={selector}}})")
            time.sleep(0.4)
            capture("downloads-compact")
            dispatch(f"hl.dsp.window.resize({{x=1180,y=820,relative=false,window={selector}}})")
            time.sleep(0.4)
            click(1080, 129)  # Pause the queue.
            capture("paused")
            click(1080, 129)  # Resume it.
            capture("resumed")
            click(80, 221)  # Settings.
            capture("settings")
            click(473, 544)  # Enable experimental parallel transfers.
            capture("settings-parallel")
            click(310, 563)  # Return to the default single stream.
            capture("settings-single")
            click(1105, 680)  # Keep encrypted sources after export.
            capture("settings-keep-originals")
            click(1105, 680)  # Restore automatic source cleanup.
            click(456, 326)  # Simplified Chinese.
            capture("settings-zh")
            click(374, 434)  # Explicit dark mode.
            expect_background(dark)
            capture("settings-dark")
            click(80, 121)
            capture("library-dark")
            click(80, 171)
            capture("downloads-dark")
            click(80, 221)
            click(436, 434)  # Explicit light mode.
            expect_background(light)
            capture("settings-light")
            click(292, 434)  # Follow the system appearance.
            capture("settings-system")
            if args.check_system_theme:
                for scheme, color in [("prefer-light", light), ("prefer-dark", dark)]:
                    run("gsettings", "set", "org.gnome.desktop.interface", "color-scheme", scheme)
                    expect_background(color)
                    capture("system-" + scheme)
                click(436, 434)  # Light override survives a dark -> light -> dark OS change.
                run("gsettings", "set", "org.gnome.desktop.interface", "color-scheme", "prefer-light")
                time.sleep(0.3)
                run("gsettings", "set", "org.gnome.desktop.interface", "color-scheme", "prefer-dark")
                time.sleep(0.3)
                expect_background(light)
                print("Verified live system theme changes and manual override", flush=True)
            click(374, 434)
            click(80, 171)  # Downloads.
            click(1080, 129)  # Pause using the localized control.
            capture("paused-zh")
            click(80, 768)  # Sign out while paused.
            capture("signed-out")

        finally:
            try:
                if old_color_scheme is not None:
                    run("gsettings", "set", "org.gnome.desktop.interface", "color-scheme", old_color_scheme)
            finally:
                if app.poll() is None:
                    app.terminate()
                    app.wait(timeout=8)
                if old_active.get("address"):
                    old_selector = json.dumps("address:" + old_active["address"])
                    dispatch(f"hl.dsp.focus({{window={old_selector}}})")
                dispatch(f"hl.dsp.cursor.move({{x={old_pointer['x']},y={old_pointer['y']}}})")


if __name__ == "__main__":
    main()
