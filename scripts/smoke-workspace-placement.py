#!/usr/bin/env python3
"""Native RPC check for insertion order, failed creation and selection continuity."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

binary = Path("target/debug/rmux").resolve()
for mode in ["end", "top", "after_current"]:
    with tempfile.TemporaryDirectory(prefix="rmux-placement-") as temporary:
        root = Path(temporary).resolve()
        config = root / "config.json"
        config.write_text(json.dumps({"new_workspace_placement": mode, "confirm_close": "never"}))
        endpoint = root / "rpc"
        env = {k: v for k, v in os.environ.items() if not k.startswith("RMUX_")}
        env.update(RMUX_CONFIG=str(config), RMUX_SHELL="/usr/bin/true", RMUX_SOCKET_DIR=str(endpoint))
        with tempfile.TemporaryFile() as log:
            app = subprocess.Popen([str(binary)], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=log)
            try:
                deadline = time.monotonic() + 8
                while not (endpoint / "rpc.sock").exists():
                    if app.poll() is not None or time.monotonic() >= deadline:
                        log.seek(0)
                        raise AssertionError("socket startup failed: " + log.read().decode(errors="replace"))
                    time.sleep(.02)
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.settimeout(8)
                    connection.connect(str(endpoint / "rpc.sock"))
                    stream = connection.makefile("rwb", buffering=0)
                    def call(method, fail=False, **params):
                        stream.write((json.dumps(dict(jsonrpc="2.0", id=1, method=method, params=params)) + "\n").encode())
                        reply = json.loads(stream.readline())
                        if fail:
                            assert "error" in reply, reply
                            return
                        assert "error" not in reply, reply
                        return reply["result"]
                    window = call("window.list")["windows"][0]["id"]
                    def rows():
                        return call("workspace.list", window_id=window)["workspaces"]
                    original = rows()[0]["id"]
                    second = call("workspace.create", window_id=window, name="Second")["id"]
                    before = rows()
                    call("workspace.create", fail=True, window_id=window, cwd=str(root / "missing"))
                    assert [(r["id"], r["selected"]) for r in rows()] == [(r["id"], r["selected"]) for r in before]
                    third = call("workspace.create", window_id=window, name="Third")["id"]
                    expected = {"end": [original, second, third], "top": [third, second, original], "after_current": [original, third, second]}[mode]
                    assert [r["id"] for r in rows()] == expected, rows()
                    assert [r["id"] for r in rows() if r["selected"]] == [original], rows()
                    fourth = call("workspace.create", window_id=window, name="Focused", focus=True)["id"]
                    assert [r["id"] for r in rows() if r["selected"]] == [fourth], rows()
                    call("workspace.close", workspace_id=fourth)
                    assert [r["id"] for r in rows()] == expected, rows()
                    if mode == "end":
                        pane = call("pane.list", workspace_id=original)["panes"][0]["id"]
                        assert call("pane.resize", pane_id=pane, direction="right")["changed"] is False
                        call("pane.split", pane_id=pane, direction="right")
                        def identity():
                            return [(r["id"], r.get("process_id")) for r in call("surface.list", workspace_id=original)["surfaces"]]
                        surfaces = identity()
                        selected = [r["id"] for r in rows() if r["selected"]]
                        assert call("pane.resize", pane_id=pane, direction="right")["changed"] is True
                        layout = next(r["layout"] for r in rows() if r["id"] == original)
                        assert abs(layout["weights"][0] - 1.1) < 1e-5 and abs(layout["weights"][1] - .9) < 1e-5, layout
                        assert identity() == surfaces
                        assert [r["id"] for r in rows() if r["selected"]] == selected
                        call("pane.resize", fail=True, pane_id=pane, direction="diagonal")
                        call("pane.resize", fail=True, pane_id="999999999", direction="left")
                        call("pane.zoom.set", pane_id=pane, zoomed=True)
                        call("pane.resize", fail=True, pane_id=pane, direction="left")
                        assert next(r["layout"] for r in rows() if r["id"] == original) == layout
                        call("pane.zoom.set", pane_id=pane, zoomed=False)
                        assert call("pane.resize", pane_id=pane, direction="left")["changed"] is True
                        print("PASS: pane.resize weights, identity, selection, single-pane no-op and rejection paths")
                    if mode == "after_current":
                        group = call("workspace.group.create", window_id=window, name="Placement", workspace_ids=[second, original])
                        call("workspace.select", workspace_id=original)
                        after_group = call("workspace.create", window_id=window, name="After group")["id"]
                        assert [r["id"] for r in rows()] == [third, second, original, after_group], rows()
                        call("workspace.group.update", window_id=window, group_id=group["id"], pinned=True)
                        after_pin = call("workspace.create", window_id=window, name="After pinned group")["id"]
                        assert [r["id"] for r in rows()] == [second, original, after_pin, third, after_group], rows()
                        assert [r["id"] for r in rows() if r["selected"]] == [original], rows()
            finally:
                if app.poll() is None:
                    app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)
    print(f"PASS: {mode} insertion, failed creation, background/focused selection and close" + (", group and pinned-group ordering" if mode == "after_current" else ""))
