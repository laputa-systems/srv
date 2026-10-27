# Agent Guide

`srv` is a Rust HTTP file browser. The program is `src/main.rs`.

- Build: `make build`
- Test: `make test`
- Check: `cargo clippy --all-targets --all-features -- --deny warnings`
- `make release TARGET=<rust-target>` writes `target/<rust-target>/dist/srv`.
- On a Linux musl target, `make` uses `musl-cargo` when it is on `PATH`, otherwise `cargo`. The `Dockerfile` image provides `musl-cargo`.
- `make verify-release` on Linux needs `readelf`. It rejects an ELF interpreter or a dynamic dependency. Other targets only check that the file exists.
- The GitHub `release` workflow is `workflow_dispatch` on the default branch. It builds macOS ARM64 and Linux x86_64 and AArch64, runs the Linux checks, and publishes prerelease `release-<sha>` with the two static Linux zips only.
- Only GET is served. The connection closes after one response.
- A directory serves `index.html` when that lookup is a file, and a listing otherwise. `.md` and `.markdown` render as HTML. A file argument is served for every URL.
- `normalize_into` rejects a URL whose `..` segments climb above the canonical root.
- A URL through a symlinked directory can still name a file outside that root. `walk_docs_dir` skips symlink entries, so the `--docs` tree does not list them.

