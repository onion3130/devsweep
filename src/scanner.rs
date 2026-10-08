use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Directory names treated as regenerable dev clutter.
pub const DEFAULT_RULES: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "__pycache__",
    ".venv",
    "venv",
    ".pytest_cache",
    ".parcel-cache",
    ".turbo",
    ".svelte-kit",
    "coverage",
];

#[derive(Debug, Clone, Serialize)]
pub struct TrashDir {
    pub path: PathBuf,
    pub rule: String,
    pub size_bytes: u64,
    pub size_human: String,
}

pub fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{} {}", n, UNITS[u])
    } else {
        format!("{:.1} {}", v, UNITS[u])
    }
}

fn is_rule(name: &str) -> Option<&'static str> {
    DEFAULT_RULES.iter().find(|r| **r == name).copied()
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok().map(|m| m.len()))
        .sum()
}

/// Scan `root` up to `max_depth` for trash dirs.
/// Skips descending into a found trash dir (its size is computed separately in parallel).
pub fn scan(root: &Path, max_depth: usize) -> Vec<TrashDir> {
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();

    let mut it = WalkDir::new(root)
        .follow_links(false)
        .max_depth(max_depth)
        .into_iter();

    // Always skip .git to stay fast
    while let Some(entry) = it.next() {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name == ".git" {
            it.skip_current_dir();
            continue;
        }
        if entry.depth() == 0 {
            continue;
        }
        if let Some(rule) = is_rule(&name) {
            candidates.push((entry.path().to_path_buf(), rule.to_string()));
            it.skip_current_dir();
        }
    }

    let mut hits: Vec<TrashDir> = candidates
        .par_iter()
        .map(|(path, rule)| {
            let size = dir_size(path);
            TrashDir {
                path: path.clone(),
                rule: rule.clone(),
                size_bytes: size,
                size_human: format_bytes(size),
            }
        })
        .collect();

    hits.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    hits
}
