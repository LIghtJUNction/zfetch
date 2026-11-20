PREFIX ?= $(HOME)/.local
DESTDIR ?=
TARGET_DIR = $(shell cargo metadata --no-deps --format-version=1 --offline | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
BINARY = $(TARGET_DIR)/release/zfetch
.PHONY: build check test benchmark install uninstall clean
build:
	cargo build --release --locked --offline
check:
	cargo fmt --check
	cargo test --lib --locked --offline
	cargo build --release --locked --offline
	cargo clippy --lib --bin zfetch --locked --offline -- -D warnings
	python3 scripts/check_binary.py "$(BINARY)"
	ZFETCH_BIN="$(abspath $(BINARY))" python3 tests/test_cli.py
test: check
benchmark: build
	python3 scripts/benchmark.py
install: build
	install -Dm755 "$(BINARY)" "$(DESTDIR)$(PREFIX)/bin/zfetch"
	install -Dm644 packaging/zfetch.1 "$(DESTDIR)$(PREFIX)/share/man/man1/zfetch.1"
	install -Dm644 completions/_zfetch "$(DESTDIR)$(PREFIX)/share/zsh/site-functions/_zfetch"
	install -Dm644 LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/zfetch/LICENSE"
uninstall:
	rm -f -- "$(DESTDIR)$(PREFIX)/bin/zfetch" "$(DESTDIR)$(PREFIX)/share/man/man1/zfetch.1" "$(DESTDIR)$(PREFIX)/share/zsh/site-functions/_zfetch" "$(DESTDIR)$(PREFIX)/share/licenses/zfetch/LICENSE"
clean:
	cargo clean
