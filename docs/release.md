# Release procedure

zfetch is distributed as native Linux archives, a Cargo package and the two Project Z AUR packages. Their version and source must agree. The executable keeps its zero-dependency, static no_std build.

## Prepare and tag

Work from a clean main checkout with the current remote changes. Set a task-specific `CARGO_TARGET_DIR` if the checkout volume is small, then run:

```sh
python3 scripts/bump.py patch --release
```

The existing entry point updates the manifest, lockfile and README installation examples. It runs `make check`, then installs the packed crate and runs the same CLI/PTY checks against that installed executable after removing the extracted source. Only after those checks succeed does it create and atomically push the version commit and annotated tag.

The tag workflow runs native x86_64 and ARM64 checks and produces both deterministic archives plus `SHA256SUMS`. Keep these release jobs enabled. Ordinary development verification is local.

## Publish to crates.io

These maintainer commands use an authenticated GitHub CLI (`gh`) to verify the release. Wait for both native release jobs and the publication job to succeed. Read the tag and release back from GitHub and verify the downloaded archive bytes against `SHA256SUMS`. From the unchanged release-tag checkout:

```sh
python3 scripts/publish_crate.py
python3 scripts/publish_crate.py --publish
```

The first command is a dry run. It may read registry metadata; offline operation is reserved for the local package and installation checks. The second repeats the packed-install checks and uploads the crate. The script requires a clean checkout, the exact local and remote tag, and a final official release containing both native archives. Cargo uses the existing local login; credentials are not copied into the repository or workflow.

After upload, check the exact version and checksum through the official registry API and perform a fresh `cargo install zfetch --version <version> --locked` into a temporary installation root. Run the existing CLI/PTY checks against that installed executable. A manifest version or successful source build alone is not publication evidence.

## Propagate the verified release

Update the source and binary AUR recipes only after downloading and hashing the real tag archive and both release archives. Regenerate their `.SRCINFO` files and build the packages locally before pushing AUR changes, as described in [the packaging guide](../packaging/aur/README.md).

Pass the immutable tag, source commit and archive checksums to the oh-my-ztty owner. Use that suite's existing pin and installation tools; do not overwrite a command inside an old suite release. Record the final command path and version after the atomic suite update. Update the Project Z snapshot through its maintainer.

## Cargo verification and the static runtime

[Cargo package verification](https://doc.rust-lang.org/cargo/commands/cargo-package.html) builds the extracted crate with the development profile. On the Linux toolchain, precompiled core code can retain an unwind-personality reference even when the application uses panic abort. zfetch supplies a terminating fallback for that reference; it does not enable stack unwinding or add a runtime library. The packed-install check exercises the normal Cargo verification step and then checks the installed release binary. See the [Rust personality implementation notes](https://doc.rust-lang.org/src/std/sys/personality/mod.rs.html) for the role of this symbol.
