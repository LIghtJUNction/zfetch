#!/usr/bin/env python3
"""Create a deterministic, per-architecture release archive."""
import argparse
import gzip
import hashlib
import io
from pathlib import Path
import tarfile
import tomllib
ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("architecture", choices=("x86_64", "aarch64"))
parser.add_argument("--binary", type=Path, default=ROOT / "target/release/zfetch")
args = parser.parse_args()
version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
name = f"zfetch-{version}-linux-{args.architecture}"
files = {
    "bin/zfetch": (args.binary, 0o755),
    "share/man/man1/zfetch.1": (ROOT / "packaging/zfetch.1", 0o644),
    "share/zsh/site-functions/_zfetch": (ROOT / "completions/_zfetch", 0o644),
    "share/licenses/zfetch/LICENSE": (ROOT / "LICENSE", 0o644),
    "README.md": (ROOT / "README.md", 0o644),
}
out = ROOT / "dist" / (name + ".tar.gz")
out.parent.mkdir(exist_ok=True)
with out.open("wb") as raw, gzip.GzipFile(fileobj=raw, filename="", mode="wb", mtime=0) as compressed:
    with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for relative, (path, mode) in sorted(files.items()):
            data = path.read_bytes()
            info = tarfile.TarInfo(name + "/" + relative)
            info.size, info.mode, info.mtime = len(data), mode, 0
            archive.addfile(info, io.BytesIO(data))
digest = hashlib.sha256(out.read_bytes()).hexdigest()
out.with_suffix(out.suffix + ".sha256").write_text(digest + "  " + out.name + "\n")
print(out)
print(digest)
