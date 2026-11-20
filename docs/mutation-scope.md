# Request-admission mutation scope

The required mutation gate (`critical` in `.cargo/mutants-gate.toml`) covers request admission. This table maps each survivor of the first run of that scope to the test that now kills it, or records why it needs none.



CI run on 147dd84 selected 154 critical mutants: 115 caught, 8 unviable, 23 missed, 8 timeout. Each survivor below is killed by the named test, was proven by applying the mutation by hand, or is an equivalent or hang case.

| Survivor (src/main.rs line at 147dd84) | Resolution |
| --- | --- |
| 776:12 `<`→`<=`, `<`→`==`; 776:30 `+`→`-`, `+`→`*` in onefile_detect | `onefile_detect_minimum_length_is_one_past_an_empty_eocd` |
| 792:37 `==`→`!=` in onefile_detect | `onefile_detect_prefers_the_real_eocd_over_a_later_signature` |
| 835:18, 835:34, 842:9, 843:9 `\|\|`→`&&` in parse_onefile_candidate | `onefile_rejects_each_multidisk_and_zip64_field` |
| 850:16 `>`→`==`/`>=`; 850:21, 850:28 `*`→`+` | `onefile_detect_central_directory_limit_is_sixteen_mib` |
| 977:34, 977:41 `*`→`+` | `onefile_detect_metadata_limit_is_eight_mib` |
| 880:21 `>`→`==`/`>=` | `onefile_detect_central_entry_may_fill_the_directory_exactly` |
| 906:35 `\|\|`→`&&` | `onefile_detect_skips_an_entry_with_either_zip64_size` |
| 933:19 `>`→`==`/`>=` | `onefile_detect_rejects_entries_that_start_at_the_central_directory` |
| 955:47 `+`→`-` | `onefile_detect_data_offset_follows_the_local_name_and_extra_field` |
| 962:24 `+`→`-` | `onefile_detect_entry_data_may_end_exactly_at_the_central_directory` |
| 858:22 `>`→`>=` | Equivalent: `cd_abs == cd_offset` is taken by the preceding branch. Excluded in `.cargo/mutants.toml`. |
| 258:15, 262:11 `+=`→`-=`/`*=` in percent_decode; 799:15 `-=`→`/=` in onefile_detect | Hang: the index never advances past the end, so the loop cannot finish (`cargo test` still running after 40s). No input can differ quickly; the gate's 3x rerun records a confirmed hang. |
| 252:13 `<`→`<=`/`==`/`>` in percent_decode | Caught: the suite fails in 0.3s locally (`percent_decode_space`, `percent_decode_slash`, integration tests). The CI timeout did not reproduce; the gate's rerun resolves it. |

## Timeout dispositions (PR #49, final)

- `src/main.rs:252:13` (`<` to `>`, `==`, `<=` in `percent_decode`): the hosted run timed out building all three at once (300 s build timeout under runner contention). A local run of cargo-mutants 27.1.0 with a 600 s build timeout found two caught and one unviable. The gate's build timeout is now 600 s.
- Five non-terminating mutants are listed in `.cargo/mutants.toml` with that reason: `258:15` and `262:11` (`+=` to `-=` and `*=` in `percent_decode`) and `799:15` (`-=` to `/=` in `onefile_detect`). No test can observe them except as a timeout. The gate's 3x replay confirmed `258:15` `-=` on PR #49. The other four ran past 40 s when applied by hand. They were never counted as confirmed hangs by the gate.
- Runner time on PR #49: 900 s (whole file, exhausted), 729 s, 734 s, and 900 s (bounded scope; the last run exhausted the budget on 150 s replays) before these dispositions.
