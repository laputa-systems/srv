Prefix: SRV
Next: 6
### SRV-5 · P3 · Establish representative serving workloads and resource measurements

- **Area:** bench
- **Where:** src/main.rs:726, src/main.rs:1071, src/main.rs:1176, src/main.rs:1501
- **Problem:** No benchmark measures completed file, directory, Markdown, docs or archive requests, startup, or resource use; regressions have no performance baseline or attribution path.
- **Recommendation:** If performance becomes a project goal, build representative checked end-to-end workloads, record CPU, memory and I/O, and add profiling and paired comparison before setting gates.
- **Fix with:** bench-pyramid
- **Effort:** M

