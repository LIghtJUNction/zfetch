#!/usr/bin/env python3
"""Interleave comparable warm runs; report stock-default fastfetch separately."""
import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import time
ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--runs", type=int, default=100)
parser.add_argument("--output", type=Path, default=ROOT / "bench-results.json")
args = parser.parse_args()
if not 10 <= args.runs <= 1000:
    parser.error("--runs must be between 10 and 1000")
binary = ROOT / "target/release/zfetch"
fastfetch = shutil.which("fastfetch")
commands = {"zfetch_text": [str(binary), "--no-color"], "zfetch_json": [str(binary), "--json"]}
if fastfetch:
    commands["fastfetch_core_json"] = [fastfetch, "--config", "none", "--logo", "none",
        "--structure", "OS:Kernel:CPU:Memory:Uptime:Shell:Terminal", "--format", "json"]
    commands["fastfetch_stock_default"] = [fastfetch, "--config", "none"]
for command in commands.values():
    for _ in range(3):
        subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, check=True, timeout=10)
samples = {name: [] for name in commands}
names = list(commands)
for i in range(args.runs):
    for name in names[i % len(names):] + names[:i % len(names)]:
        start = time.perf_counter_ns()
        subprocess.run(commands[name], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, check=True, timeout=10)
        samples[name].append((time.perf_counter_ns() - start) / 1_000_000)
def summarize(values):
    values = sorted(values)
    return {"runs": len(values), "median_ms": statistics.median(values),
            "p95_ms": values[min(len(values)-1, int(len(values)*.95))], "min_ms": values[0]}
def memory(command):
    samples = []
    for _ in range(3):
        read_fd, write_fd = os.pipe()
        fcntl.fcntl(write_fd, fcntl.F_SETPIPE_SZ, 4096)
        os.write(write_fd, b"x" * 4096)
        process = subprocess.Popen(command, stdout=write_fd, stderr=subprocess.PIPE)
        os.close(write_fd)
        blocked = False
        for _ in range(5000):
            if process.poll() is not None: break
            try: blocked = "pipe" in Path(f"/proc/{process.pid}/wchan").read_text()
            except OSError: break
            if blocked: break
            time.sleep(.001)
        values = {}
        if blocked:
            for line in Path(f"/proc/{process.pid}/smaps_rollup").read_text().splitlines():
                if line.startswith(("Rss:", "Pss:")):
                    key, value, unit = line.split()
                    assert unit == "kB"
                    values[key.rstrip(":").lower() + "_kib"] = int(value)
        while os.read(read_fd, 65536): pass
        os.close(read_fd)
        _, error = process.communicate(timeout=10)
        if process.returncode: raise RuntimeError(error.decode())
        if not blocked: return {"available": False, "reason": "could not confirm output backpressure"}
        samples.append(values)
    return {"available": True, "samples": samples,
            "rss_kib_median": statistics.median(v["rss_kib"] for v in samples),
            "pss_kib_median": statistics.median(v["pss_kib"] for v in samples)}
result = {
    "schema": "zfetch.benchmark.v1",
    "measured_at": datetime.now(timezone.utc).isoformat(),
    "architecture": platform.machine(),
    "zfetch_version": subprocess.check_output([str(binary), "--version"], text=True).strip(),
    "fastfetch_version": subprocess.check_output([fastfetch, "--version"], text=True).strip() if fastfetch else None,
    "binary_bytes": binary.stat().st_size,
    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
    "fastfetch_binary_bytes": Path(fastfetch).stat().st_size if fastfetch else None,
    "wall_scope": "Python parent spawn, execution and wait; warm filesystem cache; round-robin interleaved order; other development jobs may be active",
    "comparison_scope": "Core JSON compares OS, kernel, CPU, memory, uptime, configured/detected shell and terminal modules. Fields are not identical. Stock-default fastfetch also probes broader default modules. User fastfetch config is disabled.",
    "memory_scope": "Three /proc smaps_rollup samples per command after /proc wchan confirms blocking on a prefilled stdout pipe; resident sample, not a whole-run peak.",
    "commands": {n: [Path(c[0]).name, *c[1:]] for n, c in commands.items()},
    "wall": {name: summarize(values) for name, values in samples.items()},
    "memory": {name: memory(command) for name, command in commands.items()},
}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
