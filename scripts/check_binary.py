#!/usr/bin/env python3
"""Check static ELF and the empty production dependency graph."""
from pathlib import Path
import subprocess
import sys
import tomllib

root = Path(__file__).resolve().parents[1]
binary = Path(sys.argv[1] if len(sys.argv) > 1 else root / "target/release/zfetch")
assert binary.read_bytes().startswith(b"\x7fELF"), "not an ELF"
headers = subprocess.check_output(["readelf", "-lW", str(binary)], text=True)
dynamic = subprocess.check_output(["readelf", "-dW", str(binary)], text=True)
undefined = subprocess.check_output(["nm", "-u", str(binary)], text=True, stderr=subprocess.DEVNULL)
assert "INTERP" not in headers, "dynamic interpreter present"
assert "(NEEDED)" not in dynamic, "dynamic runtime library present"
assert not undefined.strip(), "undefined runtime symbols"
manifest = tomllib.loads((root / "Cargo.toml").read_text())
assert not manifest.get("dependencies"), "production dependencies found"
lock = tomllib.loads((root / "Cargo.lock").read_text())
assert [p["name"] for p in lock["package"]] == ["zfetch"], "unexpected dependency graph"
print(f"Static ELF verified: {binary.stat().st_size} bytes; zero Cargo dependencies, libc or dynamic loader.")
