Prefix: SRV
Next: 5

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
