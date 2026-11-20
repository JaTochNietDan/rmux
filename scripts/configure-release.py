#!/usr/bin/env python3
"""Apply checked-in public release metadata; private signing keys stay in Keychain."""
import json
import plistlib
from pathlib import Path
import sys
import tomllib

bundle = Path(sys.argv[1])
config = json.loads(Path("release/macos.json").read_text())
version = tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["package"]["version"]
path = bundle / "Contents/Info.plist"
with path.open("rb") as stream:
    info = plistlib.load(stream)
info.update(CFBundleIdentifier=config["bundle_id"], CFBundleVersion=version,
            CFBundleShortVersionString=version, LSMinimumSystemVersion=config["minimum_system_version"],
            SUFeedURL=config["feed_url"], SUPublicEDKey=config["public_ed_key"],
            SUAutomaticallyUpdate=False)
with path.open("wb") as stream:
    plistlib.dump(info, stream)
