#!/usr/bin/env python3
"""Capture native text selection and optional Fcitx5 input in a demo window.

Only the synthetic library window is captured. The script restores desktop
focus, pointer, and input-method activation state. It does not use the clipboard,
load credentials, change the input-method engine, or save application settings.
"""

import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def run(*args):
    result = subprocess.run(args, check=True, capture_output=True, text=True, timeout=15)
    if result.stdout.lstrip().startswith("error:"):
        raise RuntimeError(result.stdout)
    return result.stdout.strip()


def dispatch(expression):
    run("hyprctl", "dispatch", expression)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/endao-desktop"))
    parser.add_argument("--output", type=Path, default=Path("/tmp/endao-input-review"))
    parser.add_argument("--ime", action="store_true", help="Try nihao with the currently configured Fcitx5 engine")
    parser.add_argument("--x11", action="store_true", help="Use XWayland and XTest for a real XIM engine round trip")
    args = parser.parse_args()
    if not os.environ.get("WAYLAND_DISPLAY"):
        parser.error("Run in a Hyprland session")
    if args.ime and not args.x11:
        parser.error("--ime requires --x11; Hyprland's directed shortcuts bypass Wayland input methods")
    args.output.mkdir(parents=True, exist_ok=True)
    old_window = json.loads(run("hyprctl", "-j", "activewindow"))
    old_pointer = json.loads(run("hyprctl", "-j", "cursorpos"))
    old_ime = run("fcitx5-remote") if args.ime else None
    app = None
    try:
        with (args.output / "demo.log").open("w") as log:
            environment = dict(os.environ)
            if args.x11:
                environment.pop("WAYLAND_DISPLAY", None)
                environment["XMODIFIERS"] = "@im=fcitx"
            app = subprocess.Popen([str(args.binary.resolve()), "--demo", "library"], env=environment, stdout=log, stderr=log)
            window = None
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                if app.poll() is not None:
                    raise RuntimeError("Demo exited; see demo.log")
                windows = [window for window in json.loads(run("hyprctl", "-j", "clients")) if window["pid"] == app.pid]
                if windows:
                    window = windows[0]
                    break
                time.sleep(0.2)
            if window is None:
                raise RuntimeError("Demo window was not mapped")
            selector = json.dumps("address:" + window["address"])
            dispatch(f'hl.dsp.window.float({{action="enable",window={selector}}})')
            dispatch(f"hl.dsp.window.resize({{x=1180,y=820,relative=false,window={selector}}})")
            dispatch(f"hl.dsp.window.center({{window={selector}}})")
            dispatch(f"hl.dsp.focus({{window={selector}}})")
            time.sleep(0.4)
            x11_window = None
            if args.x11:
                x11_window = run("xdotool", "search", "--pid", str(app.pid)).splitlines()[-1]
                run("xdotool", "windowfocus", x11_window)

            def current():
                return next(window for window in json.loads(run("hyprctl", "-j", "clients")) if window["pid"] == app.pid)

            def shortcut(key, modifiers=""):
                if args.x11 and not key.startswith("mouse:"):
                    if json.loads(run("hyprctl", "-j", "activewindow")).get("pid") != app.pid:
                        raise RuntimeError("Demo lost focus; refusing to type into another app")
                    sequence = "+".join([part.lower() for part in modifiers.split()] + [key])
                    run("xdotool", "key", "--clearmodifiers", sequence)
                else:
                    dispatch(f"hl.dsp.send_shortcut({{mods={json.dumps(modifiers)},key={json.dumps(key)},window={selector}}})")
                time.sleep(0.08)

            def capture(name):
                window = current()
                x, y = window["at"]
                width, height = window["size"]
                run("grim", "-g", f"{x},{y} {width}x{height}", str(args.output / f"{name}.png"))
                print("Captured " + name, flush=True)

            window = current()
            x, y = window["at"]
            if args.x11:
                geometry = dict(line.split("=", 1) for line in run("xdotool", "getwindowgeometry", "--shell", x11_window).splitlines())
                scale_x = int(geometry["WIDTH"]) / window["size"][0]
                scale_y = int(geometry["HEIGHT"]) / window["size"][1]
                run("xdotool", "mousemove", "--window", x11_window, str(int(400 * scale_x)), str(int(140 * scale_y)))
                run("xdotool", "click", "1")
                time.sleep(0.3)
            else:
                dispatch(f"hl.dsp.cursor.move({{x={x + 400},y={y + 140}}})")
                shortcut("mouse:272")
            if args.ime:
                run("fcitx5-remote", "-c")
                time.sleep(0.2)
                print(
                    "IME disabled: "
                    + run("fcitx5-remote")
                    + " ("
                    + run("fcitx5-remote", "-n")
                    + ")",
                    flush=True,
                )
            for key in "alpha beta":
                shortcut("space" if key == " " else key)
            shortcut("Home")
            shortcut("Right", "CTRL SHIFT")
            capture("word-selection")
            shortcut("Right", "SHIFT")
            capture("extended-selection")
            shortcut("a", "CTRL")
            shortcut("BackSpace")
            if args.ime:
                run("fcitx5-remote", "-o")
                time.sleep(0.8)
                print(
                    "IME enabled: "
                    + run("fcitx5-remote")
                    + " ("
                    + run("fcitx5-remote", "-n")
                    + ")",
                    flush=True,
                )
                for key in "nihao":
                    shortcut(key)
                time.sleep(0.6)
                capture("ime-preedit")
                shortcut("space")
                time.sleep(0.5)
                capture("ime-committed")
                print(
                    "IME after commit: "
                    + run("fcitx5-remote")
                    + " ("
                    + run("fcitx5-remote", "-n")
                    + ")",
                    flush=True,
                )
                shortcut("Escape")
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
            app.wait(timeout=8)
        if old_window.get("address"):
            selector = json.dumps("address:" + old_window["address"])
            dispatch(f"hl.dsp.focus({{window={selector}}})")
        dispatch(f"hl.dsp.cursor.move({{x={old_pointer['x']},y={old_pointer['y']}}})")
        if old_ime in ("1", "2"):
            run("fcitx5-remote", "-o" if old_ime == "2" else "-c")


if __name__ == "__main__":
    main()
