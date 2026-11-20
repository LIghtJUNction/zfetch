#!/usr/bin/env python3
"""Install the packed crate, remove its source, and check the installed command."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-dirty", action="store_true")
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="1")
    command = ["cargo", "package", "--locked", "--offline"]
    if args.allow_dirty:
        command.append("--allow-dirty")
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    package = target / "package" / f"zfetch-{version}.crate"
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="zfetch-cargo-check-", dir=ROOT.parent) as temporary:
        staging = Path(temporary)
        with tarfile.open(package, "r:gz") as archive:
            members = archive.getmembers()
            prefix = f"zfetch-{version}/"
            assert all(member.name.startswith(prefix) and
                       ".." not in Path(member.name).parts and
                       (member.isfile() or member.isdir()) for member in members)
            paths = {member.name.removeprefix(prefix) for member in members}
            assert {"Cargo.toml", "Cargo.lock", "build.rs", "src/main.rs",
                    "src/lib.rs", "src/display.rs", "src/sys.rs", "LICENSE"} <= paths
            archive.extractall(staging, filter="data")
        source = staging / f"zfetch-{version}"
        installed = staging / "installed"
        install_environment = dict(environment, CARGO_TARGET_DIR=str(target / "packed-install"))
        subprocess.run(["cargo", "install", "--path", str(source), "--root", str(installed),
                        "--locked", "--offline"], cwd=ROOT, env=install_environment, check=True)
        binary = installed / "bin/zfetch"
        subprocess.run([sys.executable, str(ROOT / "scripts/check_binary.py"), str(binary)],
                       cwd=ROOT, env=environment, check=True)
        # Runtime acceptance is independent of both the source tree and PATH.
        shutil.rmtree(source)
        runtime = dict(os.environ, PATH="", TERM="xterm-256color", ZFETCH_BIN=str(binary))
        runtime.pop("NO_COLOR", None)
        result = subprocess.run([sys.executable, str(ROOT / "tests/test_cli.py")],
                                cwd=staging, env=runtime, text=True, capture_output=True)
        sys.stdout.write(result.stdout)
        sys.stderr.write(result.stderr)
        result.check_returncode()
        assert "Ran 14 tests" in result.stderr and result.stderr.rstrip().endswith("OK")
        receipt = {
            "schema": "zfetch.packed-install.v1",
            "version": version,
            "crate": {"name": package.name, "bytes": package.stat().st_size,
                      "sha256": hashlib.sha256(package.read_bytes()).hexdigest(),
                      "files": sorted(paths)},
            "installed_binary": {"bytes": binary.stat().st_size,
                                 "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()},
            "validation": {"static_elf": "passed", "cargo_dependencies": 0,
                           "cli_pty_tests": 14, "source_removed_before_cli_checks": True,
                           "runtime_path": "", "result": "passed"},
        }
    receipt_path = output / "cargo-package-validation.json"
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
