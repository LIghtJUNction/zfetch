#!/usr/bin/env python3
"""Publish only a clean, tagged source after the official binary release exists."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "LIghtJUNction/zfetch"


def read(*command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--publish", action="store_true", help="upload the verified crate")
    args = parser.parse_args()
    if read("git", "status", "--porcelain"):
        parser.error("publishing requires a clean worktree")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    tag = "v" + version
    source = read("git", "rev-parse", "HEAD")
    if read("git", "rev-parse", tag + "^{}") != source:
        parser.error("HEAD must be the exact release tag")
    remote = read("git", "ls-remote", "origin", "refs/tags/" + tag + "^{}").split()
    if not remote or remote[0] != source:
        parser.error("the published remote tag does not match HEAD")
    release = json.loads(read("gh", "api", f"repos/{REPOSITORY}/releases/tags/{tag}"))
    expected = {"SHA256SUMS", f"zfetch-{version}-linux-x86_64.tar.gz",
                f"zfetch-{version}-linux-aarch64.tar.gz"}
    if release.get("draft") or release.get("prerelease") or release.get("tag_name") != tag:
        parser.error("the official release is not final")
    if not expected <= {asset["name"] for asset in release.get("assets", [])}:
        parser.error("the official release must contain both native archives and checksums")
    subprocess.run([sys.executable, str(ROOT / "scripts/check_package.py")],
                   cwd=ROOT, check=True)
    subprocess.run(["cargo", "publish", "--dry-run", "--locked", "--offline"],
                   cwd=ROOT, check=True)
    if args.publish:
        subprocess.run(["cargo", "publish", "--locked"], cwd=ROOT, check=True)
    else:
        print("Dry run passed; --publish uploads the verified crate.")


if __name__ == "__main__":
    main()
