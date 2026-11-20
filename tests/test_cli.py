#!/usr/bin/env python3
"""Validate output, privacy defaults, live counters and terminal behavior."""
from pathlib import Path
import json
import fcntl
import re
import struct
import termios
import unicodedata
import os
import pty
import subprocess
import signal
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
        self.assertEqual(result.stderr, "")
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

    def terminal_output(self, env, args=(), columns=80):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, columns, 0, 0))
        process = subprocess.Popen([str(BINARY), *args], stdout=slave, stderr=subprocess.PIPE, env=env)
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
        self.assertIn("\x1b[1;38;2;66;133;244m", self.terminal_output(env))
        env["NO_COLOR"] = ""
        self.assertNotIn("\x1b", self.terminal_output(env))

    def test_format_selection_is_explicit_and_compatible(self):
        for args in [("--format", "json"), ("--format=json",), ("--format=text", "--json")]:
            result = self.run_cli(*args)
            self.assertEqual(set(json.loads(result.stdout)), KEYS)
            self.assertEqual(result.stderr, "")
        for args in [("--format", "text"), ("--format=text",), ("--json", "--format=text")]:
            result = self.run_cli(*args)
            self.assertIn("zfetch", result.stdout)
            self.assertFalse(result.stdout.lstrip().startswith("{"))
            self.assertNotIn("\x1b", result.stdout)
        for args in [("--format",), ("--format=xml",), ("--format", "--json"), ("--format=",)]:
            result = self.run_cli(*args, expected=2)
            self.assertEqual(result.stdout, "")
            self.assertIn("--format", result.stderr)

    def test_json_roundtrips_quotes_backslashes_and_utf8(self):
        terminal = 'terminal "quoted" \\ path 你好'
        env = dict(os.environ, TERM_PROGRAM=terminal, SHELL="/bin/a\\b")
        result = self.run_cli("--format=json", env=env)
        def unique_object(pairs):
            self.assertEqual(len(pairs), len(dict(pairs)), "duplicate JSON field")
            return dict(pairs)
        data = json.loads(result.stdout, object_pairs_hook=unique_object)
        self.assertEqual(data["terminal"], terminal)
        self.assertEqual(data["shell"], "a\\b")
        self.assertEqual(set(data), KEYS)
        self.assertEqual(result.stderr, "")
        for key in ("os", "kernel", "architecture", "cpu", "shell", "terminal"):
            self.assertTrue(data[key] is None or isinstance(data[key], str))
        for key in ("logical_cpus", "memory_total_bytes", "memory_used_bytes", "uptime_seconds"):
            self.assertTrue(data[key] is None or type(data[key]) is int)

    def test_output_failures_use_stderr_and_nonzero_status(self):
        for args in [(), ("--json",), ("--help",), ("--version",)]:
            with open("/dev/full", "wb", buffering=0) as sink:
                result = subprocess.run([str(BINARY), *args], stdout=sink, stderr=subprocess.PIPE, timeout=5)
            self.assertEqual(result.returncode, 1, args)
            self.assertIn(b"unable to write output", result.stderr)

    def test_closed_pipe_terminates_without_hanging(self):
        for restore_signals in [True, False]:
            read_fd, write_fd = os.pipe()
            os.close(read_fd)
            try:
                result = subprocess.run([str(BINARY), "--json"], stdout=write_fd,
                                        stderr=subprocess.PIPE, restore_signals=restore_signals, timeout=5)
            finally:
                os.close(write_fd)
            if restore_signals:
                self.assertEqual(result.returncode, -signal.SIGPIPE)
                self.assertEqual(result.stderr, b"")
            else:
                self.assertEqual(result.returncode, 1)
                self.assertIn(b"unable to write output", result.stderr)

    def test_dumb_terminal_never_receives_ansi(self):
        env = dict(os.environ, TERM="dumb")
        env.pop("NO_COLOR", None)
        self.assertNotIn("\x1b", self.terminal_output(env))

    def test_terminal_layout_wraps_at_actual_pty_width(self):
        env = dict(os.environ, TERM="xterm-256color", TERM_PROGRAM="你好" * 60, COLUMNS="200")
        env.pop("NO_COLOR", None)
        for columns in [1, 12, 24, 39, 40, 80]:
            text = self.terminal_output(env, columns=columns)
            visible = re.sub(r"\x1b\[[0-9;]*m", "", text)
            for line in visible.splitlines():
                cells = sum(0 if unicodedata.combining(ch) else
                            2 if unicodedata.east_asian_width(ch) in "WF" else 1 for ch in line)
                self.assertLessEqual(cells, columns, (columns, line))
            if columns >= 12:
                for label in ["OS", "Kernel", "Arch", "CPU", "Threads", "Memory", "Uptime", "Shell", "Terminal"]:
                    self.assertRegex(visible, rf"(?m)^\s*{label}(?:\s|:)")
        wide = self.terminal_output(env, columns=80)
        self.assertRegex(wide, r"\d+%")
        self.assertIn("online", wide)

    def test_json_is_not_decorated_or_wrapped_even_on_a_narrow_tty(self):
        env = dict(os.environ, TERM="xterm-256color", TERM_PROGRAM="test terminal")
        env.pop("NO_COLOR", None)
        text = self.terminal_output(env, args=("--json",), columns=12)
        self.assertNotIn("\x1b", text)
        self.assertEqual(set(json.loads(text)), KEYS)
        self.assertEqual(len(text.splitlines()), 1)

    def test_no_color_flag_preserves_terminal_layout_without_escapes(self):
        env = dict(os.environ, TERM="xterm-256color")
        env.pop("NO_COLOR", None)
        text = self.terminal_output(env, args=("--no-color",), columns=24)
        self.assertNotIn("\x1b", text)
        self.assertIn("Threads:", text)

    def test_boot_mode_has_a_separate_schema_and_bounded_evidence(self):
        data = json.loads(self.run_cli("--boot", "--json").stdout)
        self.assertEqual(data["schema"], "zfetch.boot.v1")
        self.assertEqual(data["ordering"], "structural")
        self.assertEqual([stage["stage"] for stage in data["stages"]], [
            "firmware", "bootloader", "kernel", "initramfs", "system_manager",
            "system_target", "login", "user_session", "desktop"])
        for stage in data["stages"]:
            self.assertIn(stage["status"], {"observed", "configured", "unavailable"})
            self.assertIsInstance(stage["truncated"], bool)
            for entry in stage["files"]:
                self.assertIn(entry["present"], (True, False, None))
                self.assertNotIn(str(Path.home()) + "/", entry["path"])

    def test_boot_mode_respects_the_last_output_format(self):
        self.assertEqual(json.loads(self.run_cli("--boot", "--format=text", "--json").stdout)["schema"], "zfetch.boot.v1")
        output = self.run_cli("--json", "--boot", "--format=text").stdout
        self.assertIn("zfetch boot", output)
        self.assertNotIn(chr(27), output)

if __name__ == "__main__":
    unittest.main(verbosity=2)
