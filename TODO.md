Prefix: SRV
Next: 5

### SRV-1 · P2 · Directory listings treat filenames as HTML

- **Area:** Security
- **Where:** `src/main.rs:799`, `src/main.rs:803`, `src/main.rs:809`
- **Problem:** `render_listing` percent-encodes the `href` but inserts the raw filename into the HTML body. A name such as `<img src=x onerror=alert(1)>` is served unescaped. On 2026-10-27, `target/debug/srv` listing that file returned the tag intact (`RAW_IMG True`). A browser that opens the listing runs the markup on `http://127.0.0.1:8000`, so the script can `fetch` other files in the tree and exfiltrate them. The attacker is anyone who can create a file in the served directory. Priority assumed.
- **Recommendation:** Escape `&`, `<`, and `>` in every listing text node. Keep `percent_encode` for `href`. Add a listing test whose filename contains `<` and assert the body has the escaped form and not the raw tag.
- **Fix with:** security
- **Effort:** S

### SRV-2 · P2 · index.html and docs reads follow a final symlink

- **Area:** Security
- **Where:** `src/main.rs:898`, `src/main.rs:923`, `src/main.rs:966`
- **Problem:** `dispatch` uses `symlink_metadata` and answers `403 Forbidden: symlinks not served` when the final component is a symlink. The directory index path then calls `fs::metadata` on `index.html` and `serve_file` opens it, which follows the link. `serve_docs_markdown` does the same with `fs::metadata` and `fs::read`. Probes against `target/debug/srv` on 2026-10-27: `GET /link.txt` (symlink to a file outside the root) returned 403, while `GET /` with `index.html` pointing at that file returned `200` and `secret-outside`. `GET /__docs__/md?path=evil.md` returned the outside note as HTML. `walk_docs_dir` did not list that symlink. No test covers either follow. Priority assumed.
- **Recommendation:** Stat `index.html` and the docs markdown path with `symlink_metadata` (or open with `O_NOFOLLOW`) and return the same 403 when the final component is a symlink. Leave intermediate symlink directories as they are: `GET /linked/a.txt` already serves the outside file, and README states that. Add tests for the index symlink, the docs symlink file, and the existing directory-symlink behavior so the accepted case stays.
- **Fix with:** os-boundaries
- **Effort:** S

### SRV-3 · P3 · sendfile spins when the file shrinks mid-response

- **Area:** Reliability
- **Where:** `src/main.rs:644`, `src/main.rs:678`
- **Problem:** Both `send_file` loops run while `remaining > 0` and subtract only a positive byte count. macOS `sendfile` returns `0` and writes `0` to `*len` at EOF (`man 2 sendfile`: offset past EOF is success and `0` bytes). The loop then never ends. Linux `sendfile` returns `0` at EOF and the loop subtracts nothing. A 64MiB `GET /big.bin`, truncated to `0` during the transfer, left `srv` at about 100% CPU (`TIME` `0:02.71` to `0:03.72` over one second) after the client timed out. The process is single-threaded, so it accepts nothing else until it is killed. Linux was not run. Priority assumed.
- **Recommendation:** Treat a successful `0`-byte `sendfile` as EOF and return. On macOS, still retry when `ret == -1` and the error is `EINTR`, because that can also report `0` bytes before EOF. Add a regression that truncates the file during a large response and checks that another request is still served.
- **Fix with:** os-boundaries
- **Effort:** S

### SRV-4 · P3 · Release artifact names have no check against the docs

- **Area:** Docs
- **Where:** `.github/workflows/release.yml:212`, `Makefile:34`, `README.md:9`, `AGENTS.md:7`
- **Problem:** The published names and the output directory are copied by hand. `e2ea836` stopped publishing macOS archives while `README.md` still told users to unzip `srv-aarch64-apple-darwin.zip`. After the `dist` profile split, `docs/release.md` still said `target/<rust-target>/release/`. Nothing fails if the next workflow edit does that again. The live GitHub release `release/0.5.1` (2026-03-18) still has the older asset names (`srv-0.5.1-aarch64-apple-darwin` and glibc Linux binaries); this ticket is the missing guard, not that tag. Priority assumed.
- **Recommendation:** Add a test that reads the zip paths from `.github/workflows/release.yml` and fails if those basenames are absent from `README.md`, and that `Makefile`'s `dist` output path is the path named in `AGENTS.md`.
- **Fix with:** correct
- **Effort:** S
