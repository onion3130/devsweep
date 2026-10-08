# devsweep

Blazing-fast dev clutter cleaner — find and nuke `node_modules`, `target/`, `__pycache__`, etc. in seconds.

Built in Rust for parallel filesystem scanning. Fun side-project, useful daily.

## Why

Dev folders silently eat 10-50 GB. Python/JS walkers are slow. `devsweep` walks in parallel with `rayon` + `walkdir` and skips descending into trash once found.

## Install

Requires Rust + a GCC linker on Windows (MSVC Build Tools *or* MinGW).

This repo is pinned to `stable-x86_64-pc-windows-gnu` via `rustup override`, with MinGW at `C:\mingw\mingw64\bin\gcc.exe`:

```powershell
cargo build --release
.\target\release\devsweep.exe scan ~/code
```

If you prefer MSVC:

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustup override unset
cargo build --release
```

## Usage

```powershell
# scan current dir (default)
devsweep
devsweep scan C:\code --max-depth 6

# list rules
devsweep rules

# dry-run clean (safe default)
devsweep clean C:\code

# actually delete (prompts unless --yes)
devsweep clean C:\path\to\code --delete --yes

# v0.2: pick interactively
devsweep interactive C:\path\to\code
devsweep interactive C:\path\to\code --yes

# json for scripting
devsweep scan . --json
devsweep clean . --json
```

Built-in rules: `node_modules`, `target`, `dist`, `build`, `.next`, `__pycache__`, `.venv`, `venv`, `.pytest_cache`, `.parcel-cache`, `.turbo`, `.svelte-kit`, `coverage`.

Safety:
- Never deletes source, only exact dir-name matches
- Skips `.git` always
- `clean` without `--delete` is dry-run
- Real deletes ask for confirmation unless `--yes`

## Example

```
SIZE       RULE               PATH
----------------------------------------------------------------------
9.8 KB     node_modules       C:\tmp\devsweep-test\proj1\node_modules
4.9 KB     target             C:\tmp\devsweep-test\proj1\target
----------------------------------------------------------------------
Found 2 dirs, total reclaimable: 14.6 KB
```

## Roadmap

- [x] v0.2: Interactive picker (`interactive`, e.g. `1,3` / `1-3` / `all`)
- [ ] v0.3: `--watch` daemon + weekly report
- [ ] Benchmark vs `ncdu` for README gif
- [ ] Custom `rules.toml` support

## Layout

```
src/
  main.rs     # clap CLI (scan/clean/rules)
  scanner.rs  # parallel walk + size calc
  cleaner.rs  # safe remove_dir_all
```

MIT — hack away.
