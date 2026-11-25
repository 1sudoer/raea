//! A workspace's shared/ (docs/layout.feature LY2): what every project in it imports as `@shared/<kind>/<name>`. The
//! engine's loader (engine/modules) and both bundlers (engine/bundle's script resolution, runtime/bundle's copy of an
//! edit) find it the same way. A render once left `@shared/...` to the bundler, which read it as a package and threw
//! "Can't find variable: _shared_components_badge" before the first frame, though tsc passed (0.1.7).
use std::path::{Component, Path, PathBuf};

/// The prefix a project imports its workspace's shared files by: `@shared/components/desk`.
pub const PREFIX: &str = "@shared/";

/// The shared/ of the workspace `dir` is in: under the nearest folder above it holding .raea/workspace, or that folder
/// itself when it has no shared/ yet (a workspace from before shared/ keeps its kinds at its root until `raea workspace
/// tidy`, as raea_app_registry::shared_dir reads it).
pub fn of(dir: &Path) -> Option<PathBuf> {
    let ws = dir.ancestors().find(|d| d.join(".raea/workspace").is_file())?;
    Some(if ws.join("shared").is_dir() { ws.join("shared") } else { ws.to_path_buf() })
}

/// The script a path names as an import does: the file itself, with a script or JSON extension, or a folder's index.
pub fn script(want: &Path) -> Option<PathBuf> {
    let with = |ext: &str| PathBuf::from(format!("{}{ext}", want.display()));
    let tries = [want.to_path_buf(), with(".tsx"), with(".ts"), with(".jsx"), with(".js"), with(".mjs"), with(".json"), want.join("index.tsx"), want.join("index.ts"), want.join("index.js")];
    tries.into_iter().find(|t| t.is_file())
}

/// The file `@shared/<rest>` names for a source in `dir`, or why there's none (for a message naming its importer).
pub fn file(dir: &Path, spec: &str) -> Result<PathBuf, String> {
    let rest = spec.strip_prefix(PREFIX).ok_or_else(|| format!("`{spec}` isn't an {PREFIX} import"))?;
    let Some(shared) = of(dir) else {
        return Err(format!("`{spec}` names the workspace's shared/, and {} is in no workspace (no .raea/workspace above it): import the file by a relative path, or copy it into the project", dir.display()));
    };
    for part in Path::new(rest).components() {
        if !matches!(part, Component::Normal(_)) {
            return Err(format!("`{spec}` climbs out of the workspace's shared/"));
        }
    }
    let want = shared.join(rest);
    script(&want).ok_or_else(|| format!("`{spec}` isn't there: no {} (tried .tsx, .ts, .jsx, .js, .mjs, .json and a folder's index)", want.display()))
}

/// The file an import names for a source in `dir`: a relative one beside it (None when it isn't there), an `@shared/`
/// one in the workspace's shared/ (Err when it isn't: a bundler would take the name for a package's).
pub fn import(dir: &Path, spec: &str) -> Result<Option<PathBuf>, String> {
    if spec.starts_with(PREFIX) { file(dir, spec).map(Some) } else { Ok(script(&dir.join(spec))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_import_of_shared_finds_the_workspace_s_file_from_any_depth() {
        let t = std::env::temp_dir().join(format!("raea-home-shared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        for (p, s) in [(".raea/workspace", "id = \"ws\"\n"), ("shared/components/badge.tsx", ""), ("shared/components/kit/index.ts", ""), ("projects/p/compositions/edit.tsx", "")] {
            std::fs::create_dir_all(t.join(p).parent().unwrap()).unwrap();
            std::fs::write(t.join(p), s).unwrap();
        }
        let from = t.join("projects/p/compositions");
        assert_eq!(file(&from, "@shared/components/badge").unwrap(), t.join("shared/components/badge.tsx"));
        assert_eq!(file(&from, "@shared/components/badge.tsx").unwrap(), t.join("shared/components/badge.tsx"));
        assert_eq!(file(&from, "@shared/components/kit").unwrap(), t.join("shared/components/kit/index.ts"));
        assert!(file(&from, "@shared/components/none").unwrap_err().contains("isn't there"));
        assert!(file(&from, "@shared/../projects/p/compositions/edit").unwrap_err().contains("climbs out"));
        assert!(file(&std::env::temp_dir(), "@shared/components/badge").unwrap_err().contains("in no workspace"));
        // a workspace from before shared/: its kinds at its root
        std::fs::rename(t.join("shared/components"), t.join("components")).unwrap();
        std::fs::remove_dir_all(t.join("shared")).unwrap();
        assert_eq!(file(&from, "@shared/components/badge").unwrap(), t.join("components/badge.tsx"));
        std::fs::remove_dir_all(&t).unwrap();
    }
}
