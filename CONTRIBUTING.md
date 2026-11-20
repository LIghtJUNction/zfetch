# Contributing

Read [Design](docs/design.md) before adding a probe. Keep the installed runtime no_std and free of Cargo dependencies, allocations, external commands and network access. A new field needs a bounded native source, explicit unavailable behavior and a test for malformed data. Do not add personal identity fields to default output.

Run `make check` locally. It checks formatting, parser tests, release build, Clippy, static ELF/dependency properties and CLI/PTY behavior. Use `scripts/benchmark.py` for comparable timings and retain workload descriptions.

Release tags run native x86_64 and ARM64 builds, tests and deterministic archive packaging. `python3 scripts/bump.py patch --release` validates a clean main branch, bumps the version and lockfile, runs local and packed-install checks, then atomically pushes the commit and tag. Minor and major bumps are also supported. The initial 0.1.0 tag is created after its local checks.

AUR package names must distinguish this implementation from the unrelated existing zfetch shell-script package. Use zfetch-project-z for source builds and zfetch-project-z-bin for official binaries; both provide the zfetch command and conflict with another package that owns that command. Verify downloaded release bytes before updating their checksums.

## Dependency and consumer propagation

Before completing a library upgrade or a release that affects consumers, enumerate direct and transitive consumers with the Project Z graph, including its repository-level runtime and installer edges. Run `python3 scripts/project_z.py impact --repositories zfetch` from the coordinator checkout. A new downstream must be registered in that graph together with its required-repository edge.

Coordinate each affected repository with its existing owner. Update applicable source pins, Cargo manifests and lockfiles, bundled binaries, installation scripts, package manifests and checksums. Validate the resulting consumer behavior locally, commit and push the changes, and retain the tested commit and result. Completion requires a result for every affected consumer, or a concrete blocker with a named owner and the next required action.

Known consumers are oh-my-ztty (submodule, bundled command and installation), the Project Z coordinator snapshot, and the AUR packages `zfetch-project-z` and `zfetch-project-z-bin`. The executable currently has no Cargo dependencies. Its CLI, JSON schema and archive layout still form contracts with those consumers. Coordinate work already owned by another maintainer instead of changing that checkout.

See [Release procedure](docs/release.md) for the Cargo publication gate, registry readback and consumer installation checks.
