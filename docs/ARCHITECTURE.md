# Declutter 1.0 architecture and working rules

Rust core + Tauri 2 shell + React/TypeScript UI. Behaviour is fixed by `docs/CONTRACT.md` (SPEC.md safety rules 1-11
plus the observable behaviour of the Python v0.2.2 core) and proven by the golden fixtures in `tests/golden/`.

```
crates/declutter-core/   Rust library. No UI, no Tauri. Every rule lives here.
  src/model.rs           DTOs shared with the UI (serde camelCase + ts-rs). Changing one regenerates the TypeScript.
  src/ctx.rs             Ctx = platform + cancel flag + progress sink.
  src/platform.rs        Platform trait: clock, home, known folders, drive type, Recycle Bin, data dirs.
  tests/golden/          Harness that runs the oracle fixtures against the core.
src-tauri/               Tauri 2 shell: commands, event channels, window effects, installer config.
ui/                      Vite + React + TypeScript. Talks to the shell only through ui/src/api.
tools/gen_golden.py      Generates tests/golden/*.json from the Python implementation (the oracle).
src/declutter/           The v0.2.2 Python app. Stays as the behavioural oracle until 1.0 ships.
```

## Hard rules
1. **Safety first.** SPEC.md rules 1-11 are a contract. Never weaken one, never add a path that deletes permanently,
   never act without a preview. If a change seems to need one, stop and say so.
2. **Python is the oracle.** If Rust and a fixture disagree, the Rust is wrong unless CONTRACT.md is wrong; fix the
   contract and regenerate the fixtures deliberately, in their own commit, never to make a test pass.
3. **One owner per file.** Each module names its owner in its header. Edit only your files. If you must touch someone
   else's (`model.rs`, `Cargo.toml`, `lib.rs`), keep the change minimal, additive and commit it separately.
4. **Everything platform-specific goes through `Platform`.** No direct `SHGetKnownFolderPath`, drive-type, Recycle Bin
   or clock calls outside `platform.rs`, so tests can inject a fake.
5. **No panics on user data.** Unreadable, vanishing, locked, hidden or weird files are normal. Return an error
   string or skip. `unwrap`/`expect` only on invariants you can state.
6. **One failing item never aborts a batch.** Collect messages in `ActionResult.errors`; messages are user-visible
   plain language and are part of the contract.
7. **Cancel and progress.** Long loops check `ctx.cancelled()`; long phases call `ctx.report(Progress)` with real
   `done`/`total`/`bytes`/`eta_secs`.
8. **Quality gates before every commit:** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
   No `unsafe` outside `platform.rs` and the Win32 helpers it uses, each block with a `// SAFETY:` comment.
9. **Windows-only is fine.** The app ships for Windows. Use `cfg(windows)` sparingly; the crate need not build elsewhere.
10. **Match the surrounding code's style**: comment density, naming, idiom. Comments say why, not what.

## Building on this machine
Smart App Control blocks locally compiled Rust binaries (build scripts, proc-macros, test exes) with error 4551, so
**you cannot run `cargo` locally.** Use CI as the compiler: push your branch (`git push origin <branch>`), the
`Rust` workflow builds and tests it on `windows-latest`. Read the result with
`gh run list -R w1tneess/declutter -b <branch> -w Rust -L 1` and `gh run view <id> -R w1tneess/declutter --log-failed`.
Batch your changes so each CI round trip tests as much as possible, and write carefully: compile errors cost minutes.
The UI toolchain (Node, npm, Vite) is unaffected and runs locally.

## Ownership
| Area | Files | Agent |
|---|---|---|
| Foundation | `platform.rs paths.rs walk.rs fmt.rs sort.rs undo.rs trash.rs empty.rs`, `tests/golden/ops_a.rs` | core-A |
| Analysis | `dupes.rs hashcache.rs redownloads.rs large_old.rs leftovers.rs`, `tests/golden/ops_b.rs` | core-B |
| Extras | `similar.rs sizes.rs export.rs settings.rs presets.rs reminder.rs` (own unit tests inline) | core-C |
| UI | `ui/**` | ui |
| Shell | `src-tauri/**`, root `Cargo.toml` members | shell |
| Release | `.github/workflows/**` (except rust.yml), `docs/PERF.md`, benches | release |
