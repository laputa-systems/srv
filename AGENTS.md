# Agent Guide

`srv` is a Rust HTTP file browser. The program is `src/main.rs`.

- Build: `make build`
- Test: `make test`
- Check: `cargo clippy --all-targets --all-features -- --deny warnings`
- `make release TARGET=<rust-target>` writes `target/<rust-target>/dist/srv`.
- On a Linux musl target, `make` uses `musl-cargo` when it is on `PATH`, otherwise `cargo`. The `Dockerfile` image provides `musl-cargo`.
- `make verify-release` on Linux needs `readelf`. It rejects an ELF interpreter or a dynamic dependency. Other targets only check that the file exists.
- The GitHub `release` workflow is `workflow_dispatch` on the default branch. It builds macOS ARM64 and Linux x86_64 and AArch64, runs the Linux checks, and publishes prerelease `release-<sha>` with the macOS arm64 zip and the two static Linux zips.
- Only GET is served. The connection closes after one response.
- Each connection gets its own thread, up to 64 at once; another is answered 503 and closed. A connection has 30 seconds to send its request, and a response write that blocks for 30 seconds drops it.
- A directory serves `index.html` when that lookup is a file, and a listing otherwise. `.md` and `.markdown` render as HTML. A file argument is served for every URL.
- `normalize_into` rejects a URL whose `..` segments climb above the canonical root.
- A URL through a symlinked directory can still name a file outside that root. `walk_docs_dir` skips symlink entries, so the `--docs` tree does not list them.


## Verification

| Command | Warm cost | Covers |
| --- | --- | --- |
| `make test` | under 1s (5s cold) | unit tests in `src/main.rs`, `tests/cli.rs`, `tests/help.rs` |
| `make lint` | under 1s | clippy with `--deny warnings` |

Boundaries and their guards:

- HTTP GET of files, directory listings (natural sort, encoding, escaping), `index.html`, Markdown rendering, 404, and 405: the `integration_*` tests in `src/main.rs`.
- Path containment (`..`, encoded traversal) and symlink rules: `integration_path_traversal`, `integration_encoded_traversal`, `integration_*_symlink_*`.
- `--docs` shell, tree JSON, and Markdown fragments: `integration_docs_mode_*`.
- Single-file mode: `integration_single_file_any_path`.
- A truncated send keeps the server alive: `send_file_truncated_returns_and_server_stays_live`.
- `-h`, `--help`, `--version`: `tests/help.rs` and `tests/cli.rs`.
- `-p`, `-b`, `-q`, `--docs`, a file argument, usage errors and an occupied port (exit 1), the idle-connection and connection-cap behavior: `tests/cli.rs`.
- `make coverage` reports branch coverage; `make mutants-diff` mutation-tests the diff against `BASE` and runs in CI.
- Release artifact names: `release_artifact_names_match_docs`.

Performance workload: none. srv serves one response per connection for local browsing, and no caller depends on its throughput.
