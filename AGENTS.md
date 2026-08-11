# fas-rs — Agent Notes

## Project

Rust user-space Frame Aware Scheduling daemon for Android (Magisk/KernelSU module). Requires kernel eBPF support. Target: ARM64 Android only (`aarch64-linux-android`, API 31+).

## Toolchain

- **Rust:** nightly (enforced by `rust-toolchain.toml`)
- **Build tool:** `cargo-ndk` (installs via `cargo install cargo-ndk`)
- **Required targets:** `aarch64-linux-android` (+ `armv7`, `x86_64`, `i686` for cross-check)
- **Required component:** `rust-src`
- **Android NDK:** required at build time; CI auto-downloads latest

## Key Commands (Rust)

All dev commands go through `cargo xtask`:

```bash
# Format all code (Rust + xtask)
cargo xtask format

# Lint with clippy (auto-fix)
cargo xtask lint --fix

# Check / typecheck (no binary, fast)
cargo xtask check          # debug mode
cargo xtask check -r       # release mode

# Full build → ZIP in output/
cargo xtask build           # debug
cargo xtask build -r        # release

# Clean artifacts
cargo xtask clean
```

Under the hood, `check` uses `cargo ndk --platform 31 -t arm64-v8a`; `build` uses `-Z build-std -Z trim-paths`.

## WebUI (Next.js)

Located in `webui/`. Built as part of `cargo xtask build`.

```bash
cd webui && npm install && npm run build   # production build
cd webui && npm run dev                    # local dev
```

Uses pnpm (check `pnpm-lock.yaml`). Output goes to `webui/webroot/` and gets zipped into the final package.

## Code Style

- `#![deny(clippy::all, clippy::pedantic)]` + `#![warn(clippy::nursery)]`
- Allowed lints: `module_name_repetitions`, `cast_possible_truncation`, `cast_sign_loss`, `cast_precision_loss`, `cast_possible_wrap`
- All source files need the GPL-3.0 license header (from `LICENSE_HEADER`). Managed by `licenserc.toml` (license-eye).
- Commit messages: Angular convention (see CONTRIBUTING.md)

## Architecture Quick Reference

| Path | Role |
|------|------|
| `src/main.rs` | Entry point; subcommands `merge` and `run` |
| `src/framework/` | Core logic: Config, Scheduler, modules |
| `src/cpu_common/` | CPU controller abstraction |
| `src/file_handler.rs` | File I/O helpers |
| `src/misc.rs` | Misc (e.g. `setprop`) |
| `xtask/src/` | CLI tool (`check`/`build`/`clean`/`format`/`lint`/`update`) |
| `module/` | Magisk module template (scripts, `games.toml`, META-INF) |
| `webui/` | Next.js admin UI (React + Tailwind + Radix UI) |

## Build Artifacts & Paths

- Output ZIP: `output/fas-rs({debug|release}).zip`
- Binary: `target/aarch64-linux-android/{debug|release}/fas-rs`
- User config: `/sdcard/Android/fas-rs/games.toml` (on device)
- `.gitignore`: excludes `/target`, `/output`, `/.idea`, `/__pycache__`, `.pnpm*`, `.DS_Store`

## Config Merge

Binary supports `fas-rs merge /path/to/std/profile` — prints merged TOML to stdout. Used by the Magisk module installer on each update.

## CI (GitHub Actions)

`.github/workflows/ci.yml` runs:
1. `debug-build` — builds debug variant, uploads artifact
2. `release-build` — builds release variant, uploads artifact
3. `push_to_ci_group` — posts both ZIPs to Telegram group (push-only)

CI installs NDK dynamically, uses `Swatinem/rust-cache@v2`.

## Gotchas

- `cargo xtask lint` runs clippy **twice**: debug + release mode against `aarch64-linux-android`. Both must pass.
- `build.rs` auto-generates `module/module.prop` and `update/update.json` from `Cargo.toml`. Never edit these manually.
- `mimalloc` is the global allocator with `no_thp` + `override` features — don't remove it.
- WebUI deps use `"latest"` tags in CI (`next@latest react@latest react-dom@latest`). Pin locally for stability.
