#!/usr/bin/env python3
"""Bounded native macOS split/RSS baseline; no rendering-latency claim."""
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time


def sample(panes, iteration):
    binary = Path("target/debug/rmux").resolve()
    cli = Path("target/debug/rmuxctl").resolve()
    with tempfile.TemporaryDirectory(prefix="rmux-split-bench-") as directory:
        root = Path(directory)
        shell = root / "shell"
        shell.write_text('#!/bin/sh\ntrap "exit 0" HUP TERM\nwhile IFS= read -r line; do :; done\n')
        shell.chmod(0o700)
        config = root / "config.json"
        config.write_text(json.dumps({"confirm_close": "never", "session_file": str(root / "session.json")}))
        endpoint = root / "rpc/rpc.sock"
        env = {key: value for key, value in os.environ.items() if not key.startswith("RMUX_")}
        env.update(RMUX_CONFIG=str(config), RMUX_SHELL=str(shell), RMUX_SOCKET_DIR=str(endpoint.parent))

        def call(method, **params):
            reply = subprocess.run([str(cli), "--socket", str(endpoint), method, json.dumps(params)],
                                   check=True, capture_output=True, text=True, timeout=10)
            value = json.loads(reply.stdout)
            assert "result" in value, value
            return value["result"]

        with tempfile.TemporaryFile() as log:
            app = subprocess.Popen([str(binary)], env=env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=log)
            try:
                deadline = time.monotonic() + 10
                while not endpoint.exists():
                    assert app.poll() is None, "native app exited"
                    if time.monotonic() > deadline:
                        raise TimeoutError("native app socket did not start")
                    time.sleep(.02)
                surface = call("surface.list")["surfaces"][0]
                work = surface["workspace_id"]
                leaves = [surface["pane_id"]]
                timings = []
                # Breadth-first balanced splits avoid measuring a deeply nested chain.
                for index in range(panes - 1):
                    target = leaves.pop(0)
                    start = time.perf_counter_ns()
                    created = call("pane.split", pane_id=target,
                                   direction="right" if index % 2 == 0 else "down")
                    timings.append((time.perf_counter_ns() - start) / 1_000_000)
                    leaves.extend([target, created["pane_id"]])
                surfaces = call("surface.list", workspace_id=work)["surfaces"]
                assert len(surfaces) == panes
                assert all(not row["process_exited"] for row in surfaces)
                assert len({row["process_id"] for row in surfaces}) == panes
                rss = int(subprocess.run(["/bin/ps", "-o", "rss=", "-p", str(app.pid)],
                                         check=True, capture_output=True, text=True).stdout.strip())
                result = {"panes": panes, "iteration": iteration, "split_ms": timings,
                          "split_total_ms": sum(timings), "app_rss_kib": rss}
                call("system.quit")
                app.wait(timeout=10)
                assert app.returncode == 0
                return result
            finally:
                if app.poll() is None:
                    app.terminate()
                    try:
                        app.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        app.kill()
                        app.wait(timeout=5)


if __name__ == "__main__":
    assert platform.system() == "Darwin", "This baseline currently targets macOS"
    revision = subprocess.run(["git", "rev-parse", "--short", "HEAD"], check=True,
                              capture_output=True, text=True).stdout.strip()
    for panes in [1, 8, 32]:
        samples = [sample(panes, iteration) for iteration in range(3)]
        timings = [value for row in samples for value in row["split_ms"]]
        print(json.dumps({"revision": revision, "profile": "debug", "platform": platform.platform(),
                          "panes": panes, "samples": samples,
                          "split_p50_ms": statistics.median(timings) if timings else None,
                          "split_max_ms": max(timings) if timings else None,
                          "app_rss_median_kib": statistics.median(row["app_rss_kib"] for row in samples)}), flush=True)
