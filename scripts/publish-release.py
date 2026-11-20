#!/usr/bin/env python3
"""Publish the runner-built archive before advertising it in the update feed."""
import os
from pathlib import Path
import shutil
import subprocess
import tomllib


def run(*args):
    return subprocess.run(args, check=True, text=True)

version = tomllib.loads(Path('Cargo.toml').read_text())['workspace']['package']['version']
tag = 'v' + version
repo = 'JaTochNietDan/rmux'
output = Path('target/releases')/version
archive = output/f'rmux-{version}-macos-arm64.zip'
# GitHub rejects duplicate published tags/releases; do not silently replace assets.
run('gh', 'release', 'create', tag, str(archive), str(output/'SHA256SUMS'), str(output/'appcast.xml'),
    '--repo', repo, '--target', os.environ['GITHUB_SHA'], '--title', f'rmux {version} — preview',
    '--notes-file', f'docs/releases/{tag}.md', '--prerelease')
run('git', 'fetch', 'origin', 'main')
run('git', 'checkout', '-B', 'main', 'origin/main')
Path('updates').mkdir(exist_ok=True)
shutil.copyfile(output/'appcast.xml', 'updates/appcast.xml')
run('git', 'config', 'user.name', 'github-actions[bot]')
run('git', 'config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com')
run('git', 'add', 'updates/appcast.xml')
run('git', 'commit', '-m', f'Publish Sparkle feed for {tag}')
run('git', 'push', 'origin', 'main')
