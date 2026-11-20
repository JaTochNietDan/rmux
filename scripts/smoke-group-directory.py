#!/usr/bin/env python3
"""Exercise group cwd inheritance with isolated shells and a private RPC endpoint."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

with tempfile.TemporaryDirectory(prefix="rmux-group-cwd-") as temporary:
    root = Path(temporary).resolve()
    folders = [root / name for name in ("project", "anchor-moved", "override")]
    for folder in folders:
        folder.mkdir()
    config = root / "config.json"
    config.write_text('{"confirm_close":"never"}')
    shell = root / "shell"
    shell.write_text('''#!/bin/sh
trap 'exit 0' HUP TERM
pwd -P > "$RMUX_TEST_STATE/$$.cwd"
report() { printf '\\033]7;file://localhost%s\\007' "$PWD"; }
report
while IFS= read -r line; do
    case "$line" in
        cd:*) cd "${line#cd:}" || exit 1; report ;;
    esac
done
''')
    shell.chmod(0o700)
    endpoint = root / "rpc"
    env = {k: v for k, v in os.environ.items() if not k.startswith("RMUX_")}
    env.update(RMUX_CONFIG=str(config), RMUX_SHELL=str(shell), RMUX_SOCKET_DIR=str(endpoint), RMUX_TEST_STATE=str(root))
    with tempfile.TemporaryFile() as log:
        app = subprocess.Popen([str(Path("target/debug/rmux").resolve())], env=env,
                               stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=log)
        try:
            deadline = time.monotonic() + 8
            while not (endpoint / "rpc.sock").exists():
                assert app.poll() is None, "test app exited"
                assert time.monotonic() < deadline, "socket startup timeout"
                time.sleep(.02)
            with socket.socket(socket.AF_UNIX) as connection:
                connection.settimeout(8)
                connection.connect(str(endpoint / "rpc.sock"))
                stream = connection.makefile("rwb", buffering=0)
                def call(method, **params):
                    stream.write((json.dumps(dict(jsonrpc="2.0", id=1, method=method, params=params)) + "\n").encode())
                    response = json.loads(stream.readline())
                    assert "error" not in response, response
                    return response["result"]
                window = call("window.list")["windows"][0]["id"]
                group = call("workspace.group.create", window_id=window, name="Directory test", cwd=str(folders[0]))
                group_id = group["id"]
                anchor = group["anchor_workspace_id"]
                def surface(workspace):
                    return call("surface.list", workspace_id=workspace)["surfaces"][0]
                def started_in(workspace, expected):
                    terminal = surface(workspace)
                    state = root / f"{terminal['process_id']}.cwd"
                    deadline = time.monotonic() + 4
                    while not state.exists():
                        assert time.monotonic() < deadline, "shell startup timeout"
                        time.sleep(.02)
                    assert state.read_text().strip() == str(expected), (state.read_text(), expected)
                    return terminal
                terminal = started_in(anchor, folders[0])
                call("surface.send_text", surface_id=terminal["id"], text=f"cd:{folders[1]}\n")
                deadline = time.monotonic() + 4
                while surface(anchor).get("directory_label") != str(folders[1]):
                    assert time.monotonic() < deadline, surface(anchor)
                    time.sleep(.02)
                def member(expected):
                    created = call("workspace.group.new_workspace", window_id=window, group_id=group_id)
                    started_in(created["workspace_id"], expected)
                member(folders[0])
                changed = call("workspace.group.update", window_id=window, group_id=group_id, cwd=str(folders[2]))
                assert changed["cwd"] == str(folders[2])
                member(folders[2])
                changed = call("workspace.group.update", window_id=window, group_id=group_id, clear_cwd=True)
                assert changed["cwd"] is None
                member(folders[1])
                assert surface(anchor)["process_id"] == terminal["process_id"]
                assert surface(anchor)["directory_label"] == str(folders[1])
                stream.write(b'{"jsonrpc":"2.0","method":"system.quit"}\n')
                app.wait(timeout=5)
                stream.close()
            print("PASS: fixed group cwd, anchor navigation, updated override, reset inheritance, existing shell continuity")
        finally:
            if app.poll() is None:
                app.terminate()
                try: app.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=3)
            if app.returncode:
                log.seek(0)
                print(log.read().decode(errors="replace"))
