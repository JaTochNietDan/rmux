#!/usr/bin/env python3
"""Package native GitHub runner builds without installing anything locally."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile

platform = sys.argv[1]
if platform not in ('linux', 'windows'):
    raise SystemExit('Expected linux or windows')
version = tomllib.loads(Path('Cargo.toml').read_text())['workspace']['package']['version']
name = f'rmux-{version}-{platform}-x86_64'
output = Path('target/releases') / version
output.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='rmux-package-') as temporary:
    root = Path(temporary) / name
    root.mkdir()
    suffix = '.exe' if platform == 'windows' else ''
    for binary in ('rmux', 'rmuxctl'):
        shutil.copy2(Path('target/release') / (binary + suffix), root / (binary + suffix))
    if platform == 'windows':
        shutil.copy2(Path('.deps/ghostty-vt/bin/ghostty-vt.dll'), root / 'ghostty-vt.dll')
    shutil.copytree('shell-integration', root / 'shell-integration')
    shutil.copy2('README.md', root / 'README.md')
    shutil.copy2('LICENSE', root / 'LICENSE')
    subprocess.run([sys.executable, 'scripts/release-notices.py', str(root / 'THIRD-PARTY-NOTICES.txt')], check=True)
    if platform == 'linux':
        shutil.copy2('packaging/linux/rmux.desktop', root / 'rmux.desktop')
        shutil.copy2('docs/brand/rmux.svg', root / 'rmux.svg')
        shutil.copy2('packaging/linux/README.txt', root / 'LINUX-DESKTOP.txt')
        with tarfile.open(output / (name + '.tar.gz'), 'w:gz') as archive:
            archive.add(root, arcname=name)
    else:
        with zipfile.ZipFile(output / (name + '.zip'), 'w', zipfile.ZIP_DEFLATED) as archive:
            for file in sorted(root.rglob('*')):
                if file.is_file():
                    archive.write(file, file.relative_to(root.parent))
print(f'Packaged {name}')
