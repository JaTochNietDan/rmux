"""Pack the native icon renderer's PNGs into a multi-resolution Windows ICO."""
from pathlib import Path
import struct

source = Path('target/branding/rmux.iconset')
images = [(size, (source / f'icon_{size}x{size}.png').read_bytes())
          for size in (16, 32, 128, 256)]
offset = 6 + 16 * len(images)
entries = bytearray()
payload = bytearray()
for size, data in images:
    entries.extend(struct.pack('<BBBBHHII', size % 256, size % 256, 0, 0,
                               1, 32, len(data), offset))
    payload.extend(data)
    offset += len(data)
Path('docs/brand/rmux.ico').write_bytes(
    struct.pack('<HHH', 0, 1, len(images)) + entries + payload)
