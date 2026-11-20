#!/usr/bin/env python3
"""Isolated local-socket RPC lifecycle check; never attaches to user app instances."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--app", type=Path, default=Path("target/debug/rmux"))
parser.add_argument("--client", type=Path, default=Path("target/debug/rmuxctl"))
options = parser.parse_args()

with tempfile.TemporaryDirectory(prefix="rmux-socket-") as tmp:
    root = Path(tmp)
    endpoint = root / "endpoint"
    config = root / "config.json"
    config.write_text('{"confirm_close":"never"}')
    env = {k: v for k, v in os.environ.items() if not k.startswith("RMUX_")}
    shell = root / "fixture-shell"
    shell.write_text('#!/bin/sh\ntrap "exit 0" HUP TERM\nwhile IFS= read -r line; do\n  case "$line" in\n    notify) printf "\\033]777;notify;Shell;After\\007" ;;\n    burst) printf "\\033]9;First burst\\007\\033]99;;Second burst\\007" ;;\n    ping) printf "ordinary output\\n" ;;\n  esac\ndone\n')
    shell.chmod(0o700)
    env.update(RMUX_SOCKET_DIR=str(endpoint), RMUX_CONFIG=str(config), RMUX_SHELL=str(shell))
    with tempfile.TemporaryFile() as log:
        app = subprocess.Popen([str(options.app.resolve())], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=log)
        clients = []
        try:
            deadline = time.monotonic() + 5
            while not (endpoint / "rpc.sock").exists():
                assert app.poll() is None, "test app exited before socket became ready"
                assert time.monotonic() < deadline, "socket startup timed out"
                time.sleep(.01)
            def connect():
                client = socket.socket(socket.AF_UNIX)
                client.settimeout(5)
                client.connect(str(endpoint / "rpc.sock"))
                clients.append(client)
                return client.makefile("rwb", buffering=0)
            def call(stream, method, params=None):
                stream.write((json.dumps({"jsonrpc":"2.0", "id":1, "method":method, "params":params or {}}) + "\n").encode())
                result = json.loads(stream.readline())
                assert "error" not in result, result
                return result["result"]
            cli = str(options.client.resolve())
            discovery = subprocess.run([cli, "system.capabilities"], env=env, capture_output=True, text=True, timeout=8)
            assert discovery.returncode == 0, discovery.stderr
            assert json.loads(discovery.stdout)["result"]["mode"] == "live"
            rejected = subprocess.run([cli, "--socket", str(endpoint / "rpc.sock"), "nonexistent.method"], capture_output=True, text=True, timeout=8)
            assert rejected.returncode == 1, rejected.stderr
            assert json.loads(rejected.stdout)["error"]["code"] == -32601
            first, second = connect(), connect()
            assert call(first, "system.capabilities")["mode"] == "live"
            original = call(first, "workspace.list")["workspaces"][0]
            renamed = subprocess.run([cli, "rename-workspace", "--workspace", original["id"], "Socket 世界"], env=env, capture_output=True, text=True, timeout=8)
            assert renamed.returncode == 0, renamed.stderr
            listed = subprocess.run([cli, "list-workspaces"], env=env, capture_output=True, text=True, timeout=8)
            assert listed.returncode == 0, listed.stderr
            assert json.loads(listed.stdout)["result"]["workspaces"][0]["name"] == "Socket 世界"
            assert call(second, "workspace.list")["workspaces"][0]["name"] == "Socket 世界"
            call(first, "window.create")
            all_workspaces = call(first, "workspace.list")["workspaces"]
            assert len(all_workspaces) == 2, all_workspaces
            scoped = call(first, "workspace.list", {"window_id": original["window_id"]})["workspaces"]
            assert [row["id"] for row in scoped] == [original["id"]]
            panels = subprocess.run([cli, "list-panels", "--window", original["window_id"], "--workspace", original["id"]], env=env, capture_output=True, text=True, timeout=8)
            assert panels.returncode == 0, panels.stderr
            surfaces = json.loads(panels.stdout)["result"]["surfaces"]
            assert surfaces and all(row["workspace_id"] == original["id"] for row in surfaces)
            surface = surfaces[0]["id"]
            notification = subprocess.run([cli, "notify", "--surface", surface, "--title", "Build 世界", "--body", "Complete"], env=env, capture_output=True, text=True, timeout=8)
            assert notification.returncode == 0, (notification.stdout, notification.stderr)
            old_revision = json.loads(notification.stdout)["result"]["revision"]
            newest = call(first, "notification.send", {"surface_id":surface,"body":"Review ready"})
            assert newest["revision"] > old_revision
            retained = call(first, "notification.list", {"workspace_id":original["id"]})["notifications"]
            assert [message["revision"] for message in retained] == [newest["revision"], old_revision], retained
            assert call(first, "notification.dismiss", {"surface_id":surface,"revision":old_revision})["dismissed"]
            assert not call(first, "notification.dismiss", {"surface_id":surface,"revision":old_revision})["dismissed"]
            messages = call(first, "notification.list", {"workspace_id":original["id"]})["notifications"]
            assert len(messages) == 1 and messages[0]["body"] == "Review ready", messages
            assert call(first, "notification.dismiss", {"surface_id":surface,"revision":newest["revision"]})["dismissed"]
            assert call(first, "notification.list", {"workspace_id":original["id"]})["notifications"] == []
            call(first, "surface.send_text", {"surface_id":surface,"text":"notify"})
            entered = subprocess.run([cli, "send-key", "--surface", surface, "Enter"], env=env, capture_output=True, text=True, timeout=8)
            assert entered.returncode == 0, (entered.stdout, entered.stderr)
            deadline = time.monotonic() + 3
            while True:
                messages = call(first, "notification.list", {"workspace_id":original["id"]})["notifications"]
                if messages and messages[0]["title"] == "Shell": break
                assert time.monotonic() < deadline, "shell notification did not reach UI"
                time.sleep(.01)
            assert messages[0]["revision"] > newest["revision"]
            replacement = call(first, "notification.send", {"surface_id":surface,"body":"RPC after OSC"})
            pasted = subprocess.run([cli, "send", "--surface", surface, "--paste", "ping\n"], env=env, capture_output=True, text=True, timeout=8)
            assert pasted.returncode == 0, (pasted.stdout, pasted.stderr)
            assert json.loads(pasted.stdout)["result"]["paste"] is True
            deadline = time.monotonic() + 3
            while "ordinary output" not in call(first, "surface.read_text", {"surface_id":surface})["text"]:
                assert time.monotonic() < deadline, "automation paste was not delivered"
                time.sleep(.01)
            messages = call(first, "notification.list", {"workspace_id":original["id"]})["notifications"]
            assert messages[0]["body"] == "RPC after OSC" and messages[0]["revision"] == replacement["revision"], messages

            call(first, "surface.send_text", {"surface_id":surface,"text":"burst\n"})
            deadline = time.monotonic() + 3
            while True:
                messages = call(first, "notification.list", {"workspace_id":original["id"]})["notifications"]
                if len(messages) >= 4: break
                assert time.monotonic() < deadline, "notification burst lost history"
                time.sleep(.01)
            assert messages[0]["title"] == "Second burst", messages
            assert messages[1]["body"] == "First burst", messages

            other = next(row for row in all_workspaces if row["id"] != original["id"])
            assert call(first, "pane.list", {"window_id": other["window_id"], "workspace_id": original["id"]})["panes"] == []
            invalid = subprocess.run([cli, "list-panels", "--workspace", "not-an-id"], env=env, capture_output=True, text=True, timeout=8)
            assert invalid.returncode == 1 and json.loads(invalid.stdout)["error"]["code"] == -32602
            created = subprocess.run([cli, "new-workspace", "--window", original["window_id"], "--name", "Background 世界", "--cwd", str(root)], env=env, capture_output=True, text=True, timeout=8)
            assert created.returncode == 0, (created.stdout, created.stderr)
            workspace = json.loads(created.stdout)["result"]
            assert workspace["name"] == "Background 世界" and not workspace["selected"], workspace
            scoped = call(first, "workspace.list", {"window_id": original["window_id"]})["workspaces"]
            assert len(scoped) == 2 and next(row for row in scoped if row["id"] == original["id"])["selected"]
            focused = call(first, "workspace.create", {"window_id": original["window_id"], "name":"Focused", "focus":True})
            assert focused["selected"]
            before_failure = call(first, "workspace.list", {"window_id":original["window_id"]})
            failed = subprocess.run([cli, "new-workspace", "--window", original["window_id"], "--cwd", str(root / "missing")], env=env, capture_output=True, text=True, timeout=8)
            assert failed.returncode == 1, (failed.stdout, failed.stderr)
            assert call(first, "workspace.list", {"window_id":original["window_id"]}) == before_failure
            closing_surfaces = call(first, "surface.list", {"workspace_id": workspace["id"]})["surfaces"]
            closed = subprocess.run([cli, "close-workspace", "--workspace", workspace["id"]], env=env, capture_output=True, text=True, timeout=8)
            assert closed.returncode == 0, (closed.stdout, closed.stderr)
            assert json.loads(closed.stdout)["result"]["window_closed"] is False
            remaining = call(first, "workspace.list", {"window_id":original["window_id"]})["workspaces"]
            assert len(remaining) == 2 and next(row for row in remaining if row["id"] == focused["id"])["selected"]
            assert call(first, "workspace.close", {"workspace_id":other["id"]})["window_closed"] is True
            assert len(call(first, "window.list")["windows"]) == 1
            call(first, "workspace.close", {"workspace_id":focused["id"]})
            remaining = call(first, "workspace.list")["workspaces"]
            assert len(remaining) == 1 and remaining[0]["id"] == original["id"] and remaining[0]["selected"]
            deadline = time.monotonic() + 3
            for terminal in closing_surfaces:
                pid = terminal["process_id"]
                if pid is None: continue
                while True:
                    try: os.kill(pid, 0)
                    except ProcessLookupError: break
                    assert time.monotonic() < deadline, "closed workspace shell was not reaped"
                    time.sleep(.01)
            first.close()
            clients[0].close()
            third = connect()
            assert call(third, "workspace.list")["workspaces"][0]["id"] == original["id"]
            second.write(b'{"jsonrpc":"2.0","id":2,"method":"system.quit"}\n')
            app.wait(timeout=5)
            assert not endpoint.exists(), "normal quit left socket directory behind"
            second.close()
            third.close()
            print("Socket RPC passed: bundled/client discovery and error exits, private endpoint, two clients, shared UI mutation, cross-window filters, notification/OSC coexistence, workspace creation/close and shell cleanup, reconnect, normal-quit cleanup.")
        finally:
            for client in clients: client.close()
            if app.poll() is None:
                app.terminate()
                try: app.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    app.kill(); app.wait(timeout=3)
            if app.returncode:
                log.seek(0)
                print(log.read().decode(errors="replace"))
