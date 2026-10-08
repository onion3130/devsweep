# devsweep

Blazing-fast dev clutter cleaner — find and nuke `node_modules`, `target/`, `__pycache__`, etc. in seconds.

Built in Rust for parallel filesystem scanning. Fun side-project, useful daily.

## Disclaimer — use at your own risk

devsweep **permanently deletes** directories. There is no trash or recycle-bin
recovery — deletion is irreversible. Always review `scan` output (or run
`clean` without `--delete` first, or use `interactive` to pick items one by
one), and keep backups / version control for anything important.

The authors accept **no liability** for deleted data, lost work, or any damage
from using this tool. By running `devsweep clean --delete` you accept that you
do so entirely at your own risk. See `LICENSE`.

## Why

Dev folders silently eat 10-50 GB. Python/JS walkers are slow. `devsweep` walks in parallel with `rayon` + `walkdir` and skips descending into trash once found.

## Install (beginners, one line)

Open PowerShell and run:

```powershell
irm https://raw.githubusercontent.com/onion3130/devsweep/main/install.ps1 | iex
```

That downloads the latest `devsweep.exe`, puts it in `%LOCALAPPDATA%\devsweep`,
verifies its SHA-256 checksum against the release's `SHA256SUMS.txt` (aborts
on mismatch), adds it to your PATH, and verifies it runs. Restart your
terminal, then:

```powershell
devsweep scan C:\code
devsweep interactive C:\code
```

Re-run the same line anytime to update. To uninstall, delete
`%LOCALAPPDATA%\devsweep` and remove it from your user PATH.

## Install (from source)

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

# v0.3: only old / big clutter (skips projects you're actively using)
devsweep scan C:\code --older-than 14d --min-size 100MB

# v0.3: look inside before you delete (biggest files per dir)
devsweep preview C:\code

# json for scripting
devsweep scan . --json
devsweep clean . --json
devsweep preview . --json
```

Built-in rules: `node_modules`, `target`, `dist`, `build`, `.next`, `__pycache__`, `.venv`, `venv`, `.pytest_cache`, `.parcel-cache`, `.turbo`, `.svelte-kit`, `coverage`.

Safety:
- Never deletes source, only exact dir-name matches
- Skips `.git` always
- `clean` without `--delete` is dry-run
- Real deletes ask for confirmation unless `--yes`
- `scan` shows file counts and ages; `preview` shows the biggest files inside
  each dir — look before you delete (see Disclaimer above)

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
- [x] v0.3 (in progress): Safety filters (`--older-than`, `--min-size`),
  contents preview (`preview`, file counts), liability disclaimer + `LICENSE`
- [ ] Deletion safety first: refuse dangerous roots (filesystem root, home
  dir), never follow symlinks out of the scanned tree, full test suite
  (symlinks, permissions, missing paths, nested matches, `.git` protection)
- [ ] `rules.toml` config (`[rules]` on/off per directory name, `[safety]`
  defaults) — especially useful for `build` / `dist`
- [ ] CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
  `cargo build --release`, plus Windows integration tests for deletion
- [ ] Verified installer: SHA-256-checked downloads as the default,
  checksums/signatures on every release
- [ ] Real benchmarks vs a straightforward sequential walker (evidence for
  the "blazing-fast" claim)
- [ ] `--watch` mode with periodic reclaimable-space report

## Layout

```
src/
  main.rs     # clap CLI (scan/clean/rules)
  scanner.rs  # parallel walk + size calc
  cleaner.rs  # safe remove_dir_all
```

MIT — hack away.
