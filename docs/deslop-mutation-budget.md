# Srv mutation replay cost

At `7cd8627`, [hosted run 44249399368](https://github.com/laputa-systems/srv/actions/runs/44249399368)
selected 376 identities, with 94 per shard. The initial sweeps produced
310 caught, 25 missed, 16 unviable and 25 timeouts. Both font mutants
moved from missed to caught. Five exact singleton retries timed out
again. At the 900-second deadline, the other 20 timeouts still lacked
confirmation. The original checkout and ordinary test result were
green.

The five completed retries each ran a 30-second test and spent
110–129 seconds on the build phase. Each invocation originally
started with a new Cargo target directory. The follow-up uses one
disposable source copy and target directory for timeout retries.
The initial parallel sweeps still use the standard independent
Cargo-mutants work directories.

The gate compares the native singleton selection in the replay copy
against the original result. In a local comparison the copy retained
the entire 376-identity inventory, and shard `32/376` selected only
the expected `natural_cmp` identity. Unit tests cover edits in the
copy, removed files and an out-of-tree symlink. Run them with
`python3 -B -m unittest discover -s scripts -p 'test_mutants_gate.py'`
using Python 3.11 or newer.

Run the normal hosted matrix on the updated head to measure the
remaining timeouts and cost. The 25 surviving identities still need
decisions before the whole-change candidate can qualify.

[Run 44252130017](https://github.com/laputa-systems/srv/actions/runs/44252130017)
reproduced the initial outcomes at `d785317` but rejected the replay
arguments. Cargo-mutants 27.1.0 forbids the copy-only `--gitignore`
and explicit `--jobs` flags with `--in-place`. The corrected command
uses the tool's serial in-place mode.

On macOS arm64, two exact retries in the same disposable copy
each confirmed the 30-second timeout. Native shards `32/376` and
`45/376` selected one identity apiece, and the original source
bytes were restored after each command. The first build took
77.2 seconds. The second took 4.8 with the reused target directory.
Hosted results and total runner cost remain open.
