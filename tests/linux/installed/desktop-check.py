"""The Ghostlight window on the real installation, as KDE Plasma (Wayland) sees it.

Drives the installed `ghostlight` and reads the compositor's own list of windows through KWin
scripting, so it counts the windows a person would see. It checks: Open shows one active window;
Open after minimize brings one back; closing keeps the service and Open builds the window again;
eight concurrent Opens give one window; and a service lost with the window closed comes back
without opening it. It needs KDE KWin, `qdbus6` and the user journal, and a running installation.
It closes and reopens the real window and stops the real service once (it comes back).

Run: python3 tests/linux/installed/desktop-check.py
"""
import concurrent.futures
import json
import os
import pathlib
import subprocess
import time
import uuid

BIN = pathlib.Path.home() / ".ghostlight" / "bin"
EDGE = BIN / "ghostlight"
SERVICE = BIN / "ghostlight-service"
SHELL = BIN / "ghostlight-desktop"
APP_ID = "org.sylin.ghostlight"
WORK = pathlib.Path(os.environ.get("GHOSTLIGHT_LINUX_INSTALLED_AREA", ".tmp/linux-installed")).resolve()
SETTLE = 3.0


def command(args):
    return subprocess.check_output(args, text=True, timeout=30).strip()


def kwin(body):
    """Run a KWin script body; `out(x)` hands JSON back through the user journal."""
    tag = "GLD_" + uuid.uuid4().hex[:10]
    path = WORK / f"{tag}.js"
    path.write_text("function out(x){print(%s + JSON.stringify(x));}\n" % json.dumps(tag + " ") + body)
    since = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(time.time() - 2))
    slot = command(["qdbus6", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.loadScript", str(path), tag])
    try:
        command(["qdbus6", "org.kde.KWin", f"/Scripting/Script{slot}", "org.kde.kwin.Script.run"])
        time.sleep(0.3)
    finally:
        command(["qdbus6", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.unloadScript", tag])
        path.unlink()
    log = command(["journalctl", "--user", "--since", since, "--grep", tag, "--no-pager", "-o", "cat"])
    return [json.loads(line.split(tag + " ", 1)[1]) for line in log.splitlines() if tag + " " in line]


SELECT = f"workspace.windowList().filter(w => w.resourceClass === '{APP_ID}')"


def windows():
    found = kwin(f"out({SELECT}.map(w => ({{pid: w.pid, minimized: w.minimized, "
                 "active: workspace.activeWindow === w, id: String(w.internalId)})));")
    return found[0] if found else []


def act(body):
    kwin(f"for (const w of {SELECT}) {{ {body} }} out(true);")


def processes(image):
    found = []
    for entry in pathlib.Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                if os.readlink(entry / "exe") == str(image):
                    found.append(int(entry.name))
            except OSError:
                pass
    return found


def window_word():
    return json.loads(command([str(EDGE), "status", "--json"])).get("window")


def opened():
    command([str(EDGE), "open"])
    time.sleep(SETTLE)
    shown = windows()
    assert len(shown) == 1 and shown[0]["active"] and not shown[0]["minimized"], shown
    assert len(processes(SHELL)) == 1, processes(SHELL)
    assert window_word() == "shown", window_word()
    return shown[0]


WORK.mkdir(parents=True, exist_ok=True)
checks = []
first = opened()
checks.append({"phase": "open-shows-one-active-window", "window": first})

act("w.minimized = true;")
time.sleep(1)
checks.append({"phase": "open-after-minimize-brings-one-back", "window": opened()})

service = processes(SERVICE)
act("w.closeWindow();")
time.sleep(SETTLE)
assert windows() == [] and processes(SERVICE) == service and window_word() == "tray"
checks.append({"phase": "close-keeps-the-service", "window": opened()})

with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
    list(pool.map(lambda _: command([str(EDGE), "open"]), range(8)))
time.sleep(SETTLE)
samples = []
for _ in range(10):
    shown = windows()
    assert len(shown) == 1 and shown[0]["active"] and not shown[0]["minimized"], shown
    samples.append(shown[0])
    time.sleep(0.05)
checks.append({"phase": "eight-concurrent-opens-give-one-window", "samples": samples})

act("w.closeWindow();")
time.sleep(SETTLE)
[lost] = processes(SERVICE)
os.kill(lost, 9)
deadline = time.time() + 30
while time.time() < deadline and not [pid for pid in processes(SERVICE) if pid != lost]:
    command([str(EDGE), "status"])
    time.sleep(0.5)
time.sleep(SETTLE)
assert windows() == [], "a recovered service must not open the window"
assert len(processes(SHELL)) == 1 and window_word() == "tray", (processes(SHELL), window_word())
checks.append({"phase": "a-lost-service-comes-back-without-opening-the-window"})

report = {"passed": True, "desktop": "KDE Plasma, Wayland", "bin": str(BIN), "checks": checks}
(WORK / "desktop.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps({"passed": True, "phases": [check["phase"] for check in checks]}))
