# Contributing

Read [Design](docs/design.md) before adding a probe. Keep the installed runtime no_std and free of Cargo dependencies, allocations, external commands and network access. A new field needs a bounded native source, explicit unavailable behavior and a test for malformed data. Do not add personal identity fields to default output.

Run `make check` locally. It checks formatting, parser tests, release build, Clippy, static ELF/dependency properties and CLI/PTY behavior. Use `scripts/benchmark.py` for comparable timings and retain workload descriptions.

Release tags run native x86_64 and ARM64 builds, tests and deterministic archive packaging. `python3 scripts/bump.py patch --release` validates a clean main branch, bumps the version and lockfile, runs local checks, then atomically pushes the commit and tag. Minor and major bumps are also supported. The initial 0.1.0 tag is created after its local checks.

AUR package names must distinguish this implementation from the unrelated existing zfetch shell-script package. Use zfetch-project-z for source builds and zfetch-project-z-bin for official binaries; both provide the zfetch command and conflict with another package that owns that command. Verify downloaded release bytes before updating their checksums.
