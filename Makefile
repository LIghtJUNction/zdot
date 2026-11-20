PREFIX ?= $(HOME)/.local
DESTDIR ?=
CARGO_TARGET_DIR_RESOLVED = $(shell python3 scripts/cargo_target.py)

.PHONY: build check test fmt benchmark clean
build:
	cargo build --release --locked

check:
	cargo fmt --check
	cargo test --lib --locked
	cargo build --release --locked
	python3 scripts/check_binary.py "$(CARGO_TARGET_DIR_RESOLVED)/release/zdot"
	cargo clippy --lib --bin zdot --locked -- -D warnings
	ZDOT_BIN="$(CARGO_TARGET_DIR_RESOLVED)/release/zdot" python3 tests/test_cli.py

test: check

fmt:
	cargo fmt

benchmark: build
	python3 scripts/benchmark.py

clean:
	cargo clean

.PHONY: install uninstall
install: build
	install -d "$(DESTDIR)$(PREFIX)/bin" "$(DESTDIR)$(PREFIX)/share/man/man1" "$(DESTDIR)$(PREFIX)/share/zsh/site-functions" "$(DESTDIR)$(PREFIX)/share/licenses/zdot"
	install -m755 "$(CARGO_TARGET_DIR_RESOLVED)/release/zdot" "$(DESTDIR)$(PREFIX)/bin/zdot"
	install -m644 packaging/man/zdot.1 "$(DESTDIR)$(PREFIX)/share/man/man1/zdot.1"
	install -m644 completions/_zdot "$(DESTDIR)$(PREFIX)/share/zsh/site-functions/_zdot"
	install -m644 LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/zdot/LICENSE"

uninstall:
	rm -f -- "$(DESTDIR)$(PREFIX)/bin/zdot" "$(DESTDIR)$(PREFIX)/share/man/man1/zdot.1" "$(DESTDIR)$(PREFIX)/share/zsh/site-functions/_zdot" "$(DESTDIR)$(PREFIX)/share/licenses/zdot/LICENSE"
