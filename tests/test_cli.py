#!/usr/bin/env python3
"""Validate output, privacy defaults, live counters and terminal behavior."""
from pathlib import Path
import json
import os
import pty
import subprocess
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("ZFETCH_BIN", ROOT / "target/release/zfetch"))
KEYS = {"schema", "os", "kernel", "architecture", "cpu", "logical_cpus",
        "memory_total_bytes", "memory_used_bytes", "uptime_seconds", "shell", "terminal"}

class Fetch(unittest.TestCase):
    def run_cli(self, *args, env=None, expected=0):
        result = subprocess.run([str(BINARY), *args], text=True, capture_output=True, env=env, timeout=5)
        self.assertEqual(result.returncode, expected, result.stderr)
        return result

    def test_version_and_help(self):
        version = __import__("tomllib").loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
        self.assertEqual(self.run_cli("--version").stdout, f"zfetch {version}\n")
        self.assertIn("Usage:", self.run_cli("--help").stdout)
        self.assertEqual(self.run_cli("--not-a-flag", expected=2).stdout, "")

    def test_json_schema_has_only_documented_nonidentity_fields(self):
        result = self.run_cli("--json")
        data = json.loads(result.stdout)
        self.assertEqual(set(data), KEYS)
        self.assertEqual(data["schema"], "zfetch.system.v1")
        self.assertNotIn("\x1b", result.stdout)
        self.assertIsInstance(data["memory_total_bytes"], int)
        self.assertGreater(data["memory_total_bytes"], 0)
        self.assertGreaterEqual(data["memory_used_bytes"], 0)
        self.assertLessEqual(data["memory_used_bytes"], data["memory_total_bytes"])

    def test_kernel_counters_are_read_from_the_host(self):
        before = int(float(Path("/proc/uptime").read_text().split()[0]))
        data = json.loads(self.run_cli("--json").stdout)
        after = int(float(Path("/proc/uptime").read_text().split()[0]))
        self.assertLessEqual(before, data["uptime_seconds"])
        self.assertLessEqual(data["uptime_seconds"], after)
        fields = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text().splitlines())
        self.assertEqual(data["memory_total_bytes"], int(fields["MemTotal"].split()[0]) * 1024)
        self.assertEqual(data["kernel"], os.uname().release)
        online = Path("/sys/devices/system/cpu/online").read_text().strip()
        count = 0
        for item in online.split(","):
            if "-" in item:
                a, b = map(int, item.split("-"))
                count += b - a + 1
            else:
                count += 1
        self.assertEqual(data["logical_cpus"], count)

    def test_pipe_output_is_plain_and_compact(self):
        for args in [(), ("--no-color",)]:
            text = self.run_cli(*args).stdout
            self.assertNotIn("\x1b", text)
            self.assertIn("zfetch", text)
            self.assertLessEqual(len(text.splitlines()), 15)
            for key in ["os", "kernel", "arch", "cpu", "memory", "uptime", "shell", "terminal"]:
                self.assertTrue(any(line.lstrip().startswith(key + " ") for line in text.splitlines()))

    def test_environment_is_bounded_and_cannot_inject_escapes(self):
        env = dict(os.environ, SHELL="/usr/local/bin/zsh", TERM_PROGRAM="x\x1b[31m\nz")
        data = json.loads(self.run_cli("--json", env=env).stdout)
        self.assertEqual(data["shell"], "zsh")
        self.assertEqual(data["terminal"], "x?[31m?z")
        env["TERM_PROGRAM"] = "你好" * 1000
        data = json.loads(self.run_cli("--json", env=env).stdout)
        self.assertLessEqual(len(data["terminal"].encode()), 128)
        for key in ["SHELL", "TERM_PROGRAM", "TERM"]:
            env.pop(key, None)
        data = json.loads(self.run_cli("--json", env=env).stdout)
        self.assertIsNone(data["shell"])
        self.assertIsNone(data["terminal"])

    def terminal_output(self, env):
        master, slave = pty.openpty()
        process = subprocess.Popen([str(BINARY)], stdout=slave, stderr=subprocess.PIPE, env=env)
        os.close(slave)
        data = bytearray()
        try:
            while True:
                try:
                    chunk = os.read(master, 4096)
                except OSError:
                    break
                if not chunk:
                    break
                data.extend(chunk)
            _, stderr = process.communicate(timeout=5)
        finally:
            os.close(master)
        self.assertEqual(process.returncode, 0, stderr)
        return data.decode()

    def test_truecolor_only_on_tty_and_no_color_is_respected(self):
        env = dict(os.environ)
        env.pop("NO_COLOR", None)
        self.assertIn("\x1b[38;2;212;163;115m", self.terminal_output(env))
        env["NO_COLOR"] = ""
        self.assertNotIn("\x1b", self.terminal_output(env))

if __name__ == "__main__":
    unittest.main(verbosity=2)
