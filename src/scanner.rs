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
pub struct LargestFile {
    /// Path relative to the trash dir root.
    pub rel: String,
    pub size_bytes: u64,
    pub size_human: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrashDir {
    pub path: PathBuf,
    pub rule: String,
    pub size_bytes: u64,
    pub size_human: String,
    /// Newest mtime found inside the dir, as unix seconds (0 if unknown).
    pub modified_unix: u64,
    /// Human age of the newest content, e.g. "9d ago".
    pub age_human: String,
    /// Number of files inside.
    pub files: u64,
    /// Biggest files inside (relative paths), biggest first.
    pub largest: Vec<LargestFile>,
}

/// How many of the biggest files to remember per trash dir.
pub const TOP_N: usize = 5;

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

/// Walk `path` once, returning (bytes, newest_unix, file_count, top files).
/// Only file mtimes count toward newness — a directory's own mtime changes
/// whenever entries are added/removed, which would mask old content.
fn dir_profile(path: &Path) -> (u64, u64, u64, Vec<(u64, String)>) {
    let mut bytes = 0u64;
    let mut newest = 0u64;
    let mut files = 0u64;
    let mut top: Vec<(u64, String)> = Vec::with_capacity(TOP_N);
    let entries = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok());
    for e in entries {
        let Ok(md) = e.metadata() else { continue };
        if !md.is_file() {
            continue;
        }
        files += 1;
        bytes += md.len();
        if let Ok(mtime) = md.modified() {
            if let Ok(d) = mtime.duration_since(std::time::UNIX_EPOCH) {
                newest = newest.max(d.as_secs());
            }
        }
        // Keep the TOP_N biggest files (insertion into a tiny sorted vec).
        let rel = e
            .path()
            .strip_prefix(path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| e.file_name().to_string_lossy().into_owned());
        let pos = top.iter().position(|(s, _)| md.len() > *s).unwrap_or(top.len());
        if pos < TOP_N {
            top.insert(pos, (md.len(), rel));
            top.truncate(TOP_N);
        }
    }
    // Empty dir: fall back to the dir's own mtime so it still has a real age.
    if files == 0 {
        if let Ok(md) = std::fs::metadata(path) {
            if let Ok(mtime) = md.modified() {
                if let Ok(d) = mtime.duration_since(std::time::UNIX_EPOCH) {
                    newest = d.as_secs();
                }
            }
        }
    }
    (bytes, newest, files, top)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn format_age(age_secs: u64) -> String {
    const MIN: u64 = 60;
    const HOUR: u64 = 3600;
    const DAY: u64 = 86400;
    const WEEK: u64 = 604800;
    if age_secs < MIN {
        format!("{}s ago", age_secs)
    } else if age_secs < HOUR {
        format!("{}m ago", age_secs / MIN)
    } else if age_secs < DAY {
        format!("{}h ago", age_secs / HOUR)
    } else if age_secs < WEEK {
        format!("{}d ago", age_secs / DAY)
    } else {
        format!("{}w ago", age_secs / WEEK)
    }
}

/// Parse an age like `30m`, `24h`, `14d`, `2w` (bare number = days) into seconds.
pub fn parse_age(s: &str) -> Result<u64, String> {
    let t = s.trim().to_lowercase();
    let split = t
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(t.len());
    let (num, unit) = (t[..split].trim(), t[split..].trim());
    if num.is_empty() {
        return Err(format!("bad age '{s}': expected e.g. 14d, 24h, 2w"));
    }
    let n: f64 = num
        .parse()
        .map_err(|_| format!("bad age '{s}': expected e.g. 14d, 24h, 2w"))?;
    if n < 0.0 {
        return Err(format!("bad age '{s}': must not be negative"));
    }
    let mult: f64 = match unit {
        "" | "d" | "day" | "days" => 86400.0,
        "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
        "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
        "w" | "week" | "weeks" => 604800.0,
        _ => return Err(format!("bad age '{s}': unknown unit '{unit}' (use s/m/h/d/w)")),
    };
    Ok((n * mult) as u64)
}

/// Parse a size like `500KB`, `1.5GB`, `2g` (bare number = bytes) into bytes.
pub fn parse_size(s: &str) -> Result<u64, String> {
    let t = s.trim().to_lowercase().replace('_', "");
    let split = t
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(t.len());
    let (num, unit) = (t[..split].trim(), t[split..].trim());
    if num.is_empty() {
        return Err(format!("bad size '{s}': expected e.g. 500MB, 1GB"));
    }
    let n: f64 = num
        .parse()
        .map_err(|_| format!("bad size '{s}': expected e.g. 500MB, 1GB"))?;
    if n < 0.0 {
        return Err(format!("bad size '{s}': must not be negative"));
    }
    let mult: f64 = match unit {
        "" | "b" | "byte" | "bytes" => 1.0,
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" | "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return Err(format!("bad size '{s}': unknown unit '{unit}' (use KB/MB/GB/TB)")),
    };
    Ok((n * mult) as u64)
}

/// Keep only dirs at least `older_than_secs` old and at least `min_bytes` big.
/// `None` disables that filter.
pub fn apply_filters(
    hits: Vec<TrashDir>,
    older_than_secs: Option<u64>,
    min_bytes: Option<u64>,
) -> Vec<TrashDir> {
    let now = now_unix();
    hits.into_iter()
        .filter(|h| match min_bytes {
            Some(m) => h.size_bytes >= m,
            None => true,
        })
        .filter(|h| match older_than_secs {
            Some(limit) => now.saturating_sub(h.modified_unix) >= limit,
            None => true,
        })
        .collect()
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

    let now = now_unix();
    let mut hits: Vec<TrashDir> = candidates
        .par_iter()
        .map(|(path, rule)| {
            let (size, newest, files, top) = dir_profile(path);
            let age = now.saturating_sub(newest);
            TrashDir {
                path: path.clone(),
                rule: rule.clone(),
                size_bytes: size,
                size_human: format_bytes(size),
                modified_unix: newest,
                age_human: format_age(age),
                files,
                largest: top
                    .into_iter()
                    .map(|(size_bytes, rel)| LargestFile {
                        rel,
                        size_bytes,
                        size_human: format_bytes(size_bytes),
                    })
                    .collect(),
            }
        })
        .collect();

    hits.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    hits
}

#[cfg(test)]
mod tests {
    use super::{apply_filters, format_age, parse_age, parse_size, scan, TrashDir};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_root() -> std::path::PathBuf {
        let id = TMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "devsweep-test-{}-{}",
            std::process::id(),
            id
        ))
    }

    fn hit(size: u64, modified_unix: u64) -> TrashDir {
        TrashDir {
            path: std::path::PathBuf::from("x"),
            rule: "target".into(),
            size_bytes: size,
            size_human: String::new(),
            modified_unix,
            age_human: String::new(),
            files: 0,
            largest: Vec::new(),
        }
    }

    #[test]
    fn parses_ages() {
        assert_eq!(parse_age("14d").unwrap(), 14 * 86400);
        assert_eq!(parse_age("7").unwrap(), 7 * 86400);
        assert_eq!(parse_age("24h").unwrap(), 86400);
        assert_eq!(parse_age("2w").unwrap(), 2 * 604800);
        assert_eq!(parse_age("30m").unwrap(), 1800);
        assert!(parse_age("ten").is_err());
        assert!(parse_age("5y").is_err());
    }

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("500").unwrap(), 500);
        assert_eq!(parse_size("1KB").unwrap(), 1024);
        assert_eq!(parse_size("1.5MB").unwrap(), 1572864);
        assert_eq!(parse_size("2g").unwrap(), 2 * 1024 * 1024 * 1024);
        assert!(parse_size("big").is_err());
    }

    #[test]
    fn formats_ages() {
        assert_eq!(format_age(30), "30s ago");
        assert_eq!(format_age(120), "2m ago");
        assert_eq!(format_age(7200), "2h ago");
        assert_eq!(format_age(3 * 86400), "3d ago");
        assert_eq!(format_age(3 * 604800), "3w ago");
    }

    #[test]
    fn filters_by_age_and_size() {
        let now = super::now_unix();
        let old_big = hit(10_000, now - 30 * 86400);
        let new_big = hit(10_000, now - 3600);
        let old_small = hit(10, now - 30 * 86400);
        let all = vec![old_big, new_big, old_small];
        let kept = apply_filters(all.clone(), Some(14 * 86400), None);
        assert_eq!(kept.len(), 2);
        let kept = apply_filters(all.clone(), None, Some(1000));
        assert_eq!(kept.len(), 2);
        let kept = apply_filters(all, Some(14 * 86400), Some(1000));
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn counts_files_and_ranks_largest() {
        let root = tmp_root();
        let trash = root.join("proj").join("node_modules");
        std::fs::create_dir_all(&trash).unwrap();
        std::fs::write(trash.join("small.js"), "x".repeat(10)).unwrap();
        std::fs::write(trash.join("big.js"), "x".repeat(30)).unwrap();
        std::fs::write(trash.join("mid.js"), "x".repeat(20)).unwrap();

        let hits = scan(&root, 6);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].files, 3);
        assert_eq!(hits[0].size_bytes, 60);
        let names: Vec<&str> = hits[0]
            .largest
            .iter()
            .map(|f| f.rel.as_str())
            .collect();
        assert_eq!(names, vec!["big.js", "mid.js", "small.js"]);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn skips_git_contents() {
        let root = tmp_root();
        let hidden = root.join("proj").join(".git").join("node_modules");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("x.js"), "x").unwrap();

        let hits = scan(&root, 6);
        assert!(hits.is_empty());

        std::fs::remove_dir_all(&root).unwrap();
    }
}
