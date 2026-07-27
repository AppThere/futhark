<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Fuzz-seed fixture

A parser with a planted defect, and the seed that finds it. **This crate exists
to crash.**

`check-fuzz-seeds` enforces ADR-F026: every fuzz target ships with a seeded
known-crash input proving the harness reaches the parser. Without that, "0
crashes in 24 hours" can be reported by a target wired to nothing — the
`manifest-mismatch: 0` bug at a larger scale and a longer timescale, and the
version that gets believed hardest, because nobody re-reads a green fuzz run.

The check was built before the first codec crate, which creates its own problem:
**over zero targets it would report `ok`**, and a green tick that means "nothing
to check" is the same lie one level up, inside the check built to prevent it. So
ADR-F060 requires `NOT APPLICABLE` for an empty run, and requires the check to
ship with a live subject. This is that subject.

`src/lib.rs` trusts a length prefix without checking it against the buffer —
the shape a real codec bug takes, rather than an artificial trigger, because a
seed that fires on something artificial proves the harness reaches something
artificial. `fuzz/corpus/parse_fixture/known-crash` is the input that trips it.

**Exempt from the no-panics-on-malformed-input rule by construction**, not by
oversight. It lives outside the workspace and nothing in `crates/` may depend on
it.

## What the check does

```bash
./scripts/check-fuzz-seeds
#   verified   spikes/fuzz-seed-fixture:parse_fixture: crashed as declared
```

It reads `fuzz/known-crash.toml`, finds the declared seed, and runs
`examples/replay.rs` — the same function the libFuzzer target calls, in a normal
build, because `cargo-fuzz` wants nightly and a long run while this has to be
cheap enough for every push. The target file under `fuzz/fuzz_targets/` is the
shape a real crate's would take.

Three states, reported distinctly, because collapsing them is the failure this
whole line of work is about:

| State | Meaning |
|---|---|
| `ok` | Every target ran and crashed on its seed. |
| `STRUCTURE ONLY` | Seeds and expectations are declared; no `cargo`, so nothing was shown to reach a parser. |
| `NOT APPLICABLE` | No targets exist. Not a pass — a report that nothing was checked. |

## Controls

All three run in `check-self-test`, and two are worth naming:

- **Delete the seed** — the check fails. That is the missing-paperwork case.
- **Fix the planted bug** — the check fails with `replay exited 0 — the seed did
  not crash the parser`. That is the real case: the harness no longer reaches
  the parser, and the fuzz run that follows will report zero crashes truthfully
  and meaninglessly.
