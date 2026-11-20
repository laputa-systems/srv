Prefix: DSLP
Next: 12

Deslop campaign backlog ([plan](https://github.com/laputa-systems/stuff/blob/main/docs/campaigns/deslop.md)). One agent works this file: take the lowest-numbered open ticket whose area does not start with `Needs owner ·`. After DSLP-5, work the open P1 and P2 findings, then fill in `## Report` below.

## Report

**Bench**

No benchmark harness, fixtures, results, budgets, or performance entrypoint exist in the tracked repository (Cargo.toml, Makefile, and scripts). AGENTS.md states that local browsing has no throughput requirement. No Full tier was run.

| User outcome or resource | Coverage |
| --- | --- |
| Startup and first response | None |
| Completed file and archive entry transfers, directory listings | None |
| Markdown, Mermaid and docs rendering | None |
| Concurrent requests and slow connections | None |
| CPU per completed request, peak memory, I/O, bandwidth and artifact size | None |

Ranked gap: DSLP-11 (P3) covers the absence of representative workloads, correctness and completion checks, baselines, resource metrics, and attribution. There are no existing metric scopes, sample counts, regression gates, preflight, run records, or cleanup guarantees to evaluate. CPU sampling, allocation/footprint tools, and syscall counts are candidate attribution tools; instrumented runs must remain separate from timed runs. If a performance goal is adopted, start with a checked local HTTP journey and measure CPU and memory before setting a gate. No entrypoint was added because there is no existing harness to extend, and no timing claim is made on this busy host.

Original pass complete (base `dcc7921`); its tickets were closed.

| Measure | Before | After |
| --- | --- | --- |
| Production lines (`src/main.rs` before `#[cfg(test)]`) | 1,020 | 1,058 |
| Test lines (`tests/`) | 29 | 182 |
| Warm `make test` | under 1s | 0.4s |
| Line / branch coverage | 89.5% / 66.8% | 95.5% / 87.9% |
| Mutants caught / missed / timeout / unviable | 162 / 61 / 8 / 8 (239) | 192 / 0 / 12 / 8 (214), after the DSLP-9 fix killed the last 2 |
| Escape hatches (`unsafe`, `allow`) | 3 | 0 |
| Findings fixed | | P2: DSLP-6, DSLP-7, DSLP-9; P3: DSLP-8 |

Per step:

- **Ground:** no reds; the CLI flags had no test.
- **Diagnose:** three defects, each reproduced against the binary: an idle connection stalled every request, more than 16 headers got 400, and the `sendfile` FFI was hand-declared.
- **Types:** `unsafe_code = "forbid"` adopted; five Clippy candidates rejected as false positives.
- **Tests:** CLI and HTTP boundary tests; the mutation run found a fourth defect (no response for a path beneath a file) and two weak oracles (`--version`, `--docs`). A free-port race in the new tests became `-p 0` support in the banner.
- **Dead code:** no unused dependencies. Cuts: a one-use helper, a redundant guard, and a Windows-separator rewrite that broke backslash file names.
- **Findings:** each fix had a test that failed first. The CI mutation lane caught one undertested fix (#29, followed by #30).
- **Report review:** an independent reviewer (bridge worker) read the cumulative diff and found that the idle-connection fix had left connection threads unbounded, with spawn able to panic and writes untimed. Fixed in #36 with a 64-connection cap, 503 beyond it, and a write timeout.

Method feedback:

- **The mutation lane:** a test-only diff mutates the whole crate, and index-loop mutants then time out; exit 3 must count as detected. Keep the timeout near ten times the suite's run time.
- **Merging:** merge only after every check has reported, the non-required lane included.
- **Backlog edits:** closing tickets in parallel PRs conflicts in this file; work findings serially.
- **Independent review:** keep it. It found a regression that the fix's own tests and the mutation lane could not, because no test bounded resource use.

Shared-file gaps: F3 should generate the `coverage` and `mutants-diff` targets and the lane, because every Rust repository needs the same Makefile block.

## Tickets

### DSLP-11 · P3 · Establish representative serving workloads and resource measurements

- **Area:** deslop · bench
- **Where:** src/main.rs:726, src/main.rs:1071, src/main.rs:1176, src/main.rs:1501
- **Problem:** No benchmark measures completed file, directory, Markdown, docs or archive requests, startup, or resource use; regressions have no performance baseline or attribution path.
- **Recommendation:** If performance becomes a project goal, build representative checked end-to-end workloads, record CPU, memory and I/O, and add profiling and paired comparison before setting gates.
- **Fix with:** bench-pyramid
- **Effort:** M

