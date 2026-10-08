mod cleaner;
mod scanner;

use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "devsweep", version, about = "Blazing-fast dev clutter cleaner")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Root to scan when no subcommand is given
    #[arg(global = false)]
    path: Option<PathBuf>,

    /// Max scan depth (default: 6)
    #[arg(long, global = true, default_value_t = 6)]
    max_depth: usize,

    /// Only include dirs untouched for at least this long (e.g. 14d, 24h, 2w)
    #[arg(long, global = true)]
    older_than: Option<String>,

    /// Only include dirs at least this big (e.g. 100MB, 1GB)
    #[arg(long, global = true)]
    min_size: Option<String>,

    /// Output as JSON
    #[arg(long, global = true, default_value_t = false)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan for dev clutter (default)
    Scan {
        /// Root to scan
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Show what would be deleted (safe) or delete it
    Clean {
        /// Root to clean
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Only show, don't delete (default behaviour, wins over --delete)
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Actually delete (permanent and irreversible — see Disclaimer in README)
        #[arg(long, default_value_t = false)]
        delete: bool,

        /// Skip confirmation prompt
        #[arg(long, short, default_value_t = false)]
        yes: bool,
    },
    /// List the built-in rules
    Rules,
    /// Pick which dirs to delete (numbered checklist)
    Interactive {
        /// Root to clean
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Skip confirmation prompt
        #[arg(long, short, default_value_t = false)]
        yes: bool,
    },
    /// Show what's inside each trash dir (biggest files first)
    Preview {
        /// Root to preview
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let older_than = match &cli.older_than {
        Some(s) => match scanner::parse_age(s) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(2);
            }
        },
        None => None,
    };
    let min_size = match &cli.min_size {
        Some(s) => match scanner::parse_size(s) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(2);
            }
        },
        None => None,
    };

    match cli.command {
        Some(Commands::Rules) => {
            println!("Built-in trash dir names:");
            for r in scanner::DEFAULT_RULES {
                println!("  {r}");
            }
        }
        Some(Commands::Scan { path }) => {
            run_scan(&path, cli.max_depth, cli.json, older_than, min_size);
        }
        Some(Commands::Clean {
            path,
            dry_run,
            delete,
            yes,
        }) => {
            // `clean` -> dry run, `clean --delete` -> delete, `--dry-run` always wins
            let do_delete = delete && !dry_run;
            run_clean(&path, cli.max_depth, cli.json, do_delete, yes, older_than, min_size);
        }
        Some(Commands::Interactive { path, yes }) => {
            run_interactive(&path, cli.max_depth, yes, older_than, min_size);
        }
        Some(Commands::Preview { path }) => {
            run_preview(&path, cli.max_depth, cli.json, older_than, min_size);
        }
        None => {
            let root = cli.path.unwrap_or(PathBuf::from("."));
            run_scan(&root, cli.max_depth, cli.json, older_than, min_size);
        }
    }
}

fn run_scan(
    root: &PathBuf,
    max_depth: usize,
    as_json: bool,
    older_than: Option<u64>,
    min_size: Option<u64>,
) {
    let mut hits = scanner::apply_filters(scanner::scan(root, max_depth), older_than, min_size);
    // scan() already sorts biggest-first
    let total: u64 = hits.iter().map(|h| h.size_bytes).sum();

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&hits).unwrap_or_else(|_| "[]".into())
        );
        return;
    }

    if hits.is_empty() {
        println!("No dev clutter found under {}", root.display());
        return;
    }

    // Simple table without extra deps
    println!(
        "{:<10} {:<8} {:>7} {:<12} {}",
        "SIZE", "AGE", "FILES", "RULE", "PATH"
    );
    println!("{}", "-".repeat(80));
    for h in &hits {
        println!(
            "{:<10} {:<8} {:>7} {:<12} {}",
            scanner::format_bytes(h.size_bytes),
            h.age_human,
            h.files,
            h.rule,
            h.path.display()
        );
    }
    println!("{}", "-".repeat(70));
    println!(
        "Found {} dirs, total reclaimable: {}",
        hits.len(),
        scanner::format_bytes(total)
    );
    println!("\nRun `devsweep clean <path> --delete` to remove them (dry-run by default).");
    let _ = &mut hits;
}

fn run_clean(
    root: &PathBuf,
    max_depth: usize,
    as_json: bool,
    do_delete: bool,
    yes: bool,
    older_than: Option<u64>,
    min_size: Option<u64>,
) {
    let hits = scanner::apply_filters(scanner::scan(root, max_depth), older_than, min_size);
    if hits.is_empty() {
        println!("Nothing to clean under {}", root.display());
        return;
    }
    let total: u64 = hits.iter().map(|h| h.size_bytes).sum();

    if as_json {
        let out = serde_json::json!({
            "dry_run": !do_delete,
            "total_bytes": total,
            "total_human": scanner::format_bytes(total),
            "targets": hits,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        if !do_delete {
            return;
        }
    } else if !do_delete {
        println!("DRY RUN — would free {} in {} dirs:", scanner::format_bytes(total), hits.len());
        for h in &hits {
            println!("  {}  {}", scanner::format_bytes(h.size_bytes), h.path.display());
        }
        println!("\nRe-run with `--delete` to actually remove. Add `--yes` to skip confirm.");
        return;
    }

    // Real delete: confirm unless --yes or --json (script mode still confirms unless yes)
    if !yes && !as_json {
        print!(
            "Delete {} dirs freeing {} under {}? [y/N] ",
            hits.len(),
            scanner::format_bytes(total),
            root.display()
        );
        let _ = io::stdout().flush();
        let mut ans = String::new();
        if io::stdin().read_line(&mut ans).is_ok() {
            let ans = ans.trim().to_lowercase();
            if ans != "y" && ans != "yes" {
                println!("Aborted.");
                return;
            }
        }
    }

    let freed = cleaner::remove_all(&hits);
    println!(
        "Deleted {} dirs, freed {}",
        hits.len(),
        scanner::format_bytes(freed)
    );
}

fn parse_selection(input: &str, len: usize) -> Vec<usize> {
    let s = input.trim().to_lowercase();
    if s == "all" {
        return (0..len).collect();
    }
    if s.is_empty() || s == "none" || s == "q" || s == "quit" {
        return Vec::new();
    }
    let mut out: Vec<usize> = Vec::new();
    for part in s.split(|c| c == ',' || c == ' ' || c == ';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // Support ranges like 1-3
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(mut lo), Ok(mut hi)) =
                (a.trim().parse::<usize>(), b.trim().parse::<usize>())
            {
                if lo >= 1 && hi >= 1 {
                    if lo > hi {
                        std::mem::swap(&mut lo, &mut hi);
                    }
                    for n in lo..=hi {
                        if n >= 1 && n <= len && !out.contains(&(n - 1)) {
                            out.push(n - 1);
                        }
                    }
                }
                continue;
            }
        }
        if let Ok(n) = part.parse::<usize>() {
            if n >= 1 && n <= len && !out.contains(&(n - 1)) {
                out.push(n - 1);
            }
        }
    }
    out.sort_unstable();
    out
}

fn run_interactive(
    root: &PathBuf,
    max_depth: usize,
    yes: bool,
    older_than: Option<u64>,
    min_size: Option<u64>,
) {
    let hits = scanner::apply_filters(scanner::scan(root, max_depth), older_than, min_size);
    if hits.is_empty() {
        println!("Nothing to clean under {}", root.display());
        return;
    }

    println!("Select dirs to delete under {}:", root.display());
    for (i, h) in hits.iter().enumerate() {
        println!(
            "  [{}] {:<10} {:>6} files {:<8} {:<12} {}",
            i + 1,
            scanner::format_bytes(h.size_bytes),
            h.files,
            h.age_human,
            h.rule,
            h.path.display()
        );
    }
    print!("\nEnter numbers (e.g. 1,3 or 1-3 or all/none): ");
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        println!("Aborted.");
        return;
    }
    let idx = parse_selection(&input, hits.len());
    if idx.is_empty() {
        println!("Nothing selected.");
        return;
    }

    let selected: Vec<_> = idx.iter().map(|&i| hits[i].clone()).collect();
    let total: u64 = selected.iter().map(|h| h.size_bytes).sum();
    println!(
        "Selected {} dirs, {} reclaimable:",
        selected.len(),
        scanner::format_bytes(total)
    );
    for h in &selected {
        println!("  {}  {}", scanner::format_bytes(h.size_bytes), h.path.display());
    }

    if !yes {
        print!("Delete these? [y/N] ");
        let _ = io::stdout().flush();
        let mut ans = String::new();
        if io::stdin().read_line(&mut ans).is_ok() {
            let ans = ans.trim().to_lowercase();
            if ans != "y" && ans != "yes" {
                println!("Aborted.");
                return;
            }
        }
    }

    let freed = cleaner::remove_all(&selected);
    println!(
        "Deleted {} dirs, freed {}",
        selected.len(),
        scanner::format_bytes(freed)
    );
}

fn run_preview(
    root: &PathBuf,
    max_depth: usize,
    as_json: bool,
    older_than: Option<u64>,
    min_size: Option<u64>,
) {
    let hits = scanner::apply_filters(scanner::scan(root, max_depth), older_than, min_size);
    if hits.is_empty() {
        println!("No dev clutter found under {}", root.display());
        return;
    }

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&hits).unwrap_or_else(|_| "[]".into())
        );
        return;
    }

    for h in &hits {
        println!(
            "{}  ({} in {} files, {}, {})",
            h.path.display(),
            h.size_human,
            h.files,
            h.rule,
            h.age_human
        );
        if h.largest.is_empty() {
            println!("    (empty directory)");
        }
        for f in &h.largest {
            println!("    {:<10} {}", f.size_human, f.rel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_selection;

    #[test]
    fn picks_numbers_and_ranges() {
        assert_eq!(parse_selection("1,3", 5), vec![0, 2]);
        assert_eq!(parse_selection("1-3", 5), vec![0, 1, 2]);
        assert_eq!(parse_selection("all", 3), vec![0, 1, 2]);
        assert_eq!(parse_selection("none", 3), Vec::<usize>::new());
        assert_eq!(parse_selection("9,0,abc", 3), Vec::<usize>::new());
    }
}
