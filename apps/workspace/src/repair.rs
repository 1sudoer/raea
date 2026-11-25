//! Workspaces raea 0.1.5 and 0.1.6 broke on Windows (#169): their include.path is a relative climb glued to a `\\?\`
//! path (`../../../../\\?\C:/\/Users/...`), which Git for Windows can't open, so every git command in the repo failed
//! with "Invalid argument", `git rev-parse` too. A repair can't ask git, then: it reads the repo's config file itself
//! and drops those lines, and the include is added again the right way (workspace::include, run by `raea workspace
//! init` inside it, `raea workspace adopt` and `raea doctor --fix`).

use std::path::{Path, PathBuf};

/// The repo's own config file, found without git: `.git/config`, or the config in the folder a `.git` file names (a
/// submodule's, under the workspace's .git/modules).
fn config_file(repo: &Path) -> Option<PathBuf> {
    let dot = repo.join(".git");
    if dot.is_dir() {
        return Some(dot.join("config"));
    }
    let text = std::fs::read_to_string(&dot).ok()?;
    let dir = PathBuf::from(text.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim());
    Some(if dir.is_absolute() { dir } else { repo.join(dir) }.join("config"))
}

/// The config's text without the `path = ...\\?\...` lines of its [include] sections, and whether there were any.
/// [includeIf] sections and every other line are kept as they are.
pub(crate) fn without_verbatim(text: &str) -> (String, bool) {
    let mut section = String::new();
    let mut dropped = false;
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| {
            let t = l.trim();
            if t.starts_with('[') {
                section = t.trim_start_matches('[').split(']').next().unwrap_or_default().trim().to_ascii_lowercase();
                return true;
            }
            let key = t.split('=').next().unwrap_or_default().trim();
            let bad = section == "include" && key.eq_ignore_ascii_case("path") && t.contains("?\\");
            dropped |= bad;
            !bad
        })
        .collect();
    (kept.join(eol) + eol, dropped)
}

/// Whether the repo's config holds an include git can't open on Windows (what `raea doctor` reports).
pub fn has_verbatim_include(repo: &Path) -> bool {
    config_file(repo).and_then(|c| std::fs::read_to_string(c).ok()).is_some_and(|t| without_verbatim(&t).1)
}

/// Drops those includes from the repo's config. Ok(true) when there were some.
pub(crate) fn drop_verbatim_includes(repo: &Path) -> Result<bool, String> {
    let Some(cfg) = config_file(repo) else { return Ok(false) };
    let Ok(text) = std::fs::read_to_string(&cfg) else { return Ok(false) };
    let (fixed, dropped) = without_verbatim(&text);
    if dropped {
        std::fs::write(&cfg, fixed).map_err(|e| format!("{}: {e}", cfg.display()))?;
    }
    Ok(dropped)
}
