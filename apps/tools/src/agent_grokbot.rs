//! Grok Bot (xAI's desktop agent, made by Anysphere: /Applications/Grok Bot.app, bundle com.anysphere.sand) as a
//! `raea skills install --agent grokbot` target (docs/skills.feature SK13). Read from its app.asar, 1.1.0 (11-14):
//! Grok Bot keeps nothing on this Mac that raea could write. Its agent runs on a computer of its own in the cloud (its
//! "box"), its skills are held there, and its MCP servers belong to the person's account (Settings → Integrations →
//! Add a custom connector), a server's command running on that computer, not here. It has no hooks, no CLI and no ACP.
//! What does reach this Mac is its Execution on this computer: the Bot runs a command here, through its local daemon.
//! So raea stages its skill in a folder of its own, stamped and with a preface saying where raea runs, and gives the
//! grokbot:// links that finish it in Grok Bot: the Execution setting, and a message to the Bot, filled in but not
//! sent, asking it to make the skill from the staged file ("Ask your Bot to create one", as Grok Bot itself says).
use std::path::{Path, PathBuf};

type R<T> = Result<T, String>;

/// The folder raea stages Grok Bot's copy in: $RAEA_GROKBOT_HOME, else grokbot in raea's own folder in the home.
pub fn home() -> R<PathBuf> {
    match std::env::var_os("RAEA_GROKBOT_HOME").filter(|v| !v.is_empty()) {
        Some(d) => Ok(PathBuf::from(d)),
        None => Ok(crate::agent_mcp::user_home()?.join(".raea").join("grokbot")),
    }
}

/// Where the staged skills go, one folder per skill holding SKILL.md, as for every agent.
pub fn skills_dir() -> R<PathBuf> {
    Ok(home()?.join("skills"))
}

/// The app, where macOS keeps it (/Applications, or ~/Applications).
pub fn app() -> Option<PathBuf> {
    let home = crate::agent_mcp::user_home().ok();
    [Some(PathBuf::from("/Applications")), home.map(|h| h.join("Applications"))].into_iter().flatten().map(|d| d.join("Grok Bot.app")).find(|a| a.join("Contents/Info.plist").is_file())
}

/// The largest SKILL.md Grok Bot's Skills → Add new skill → Upload file takes, in bytes (256 KB).
pub const MAX_SKILL: usize = 256_000;

/// After `raea skills install --agent grokbot` has copied raea's skills into `dir` (`said`, what it said doing so): the
/// preface in raea's SKILL.md, and the links that finish it. A linked folder is the person's, left as it is. Grok Bot
/// gets no hooks (it has none) and no MCP server (its command would run on the Bot's own computer, not this Mac).
pub fn stage(dir: &Path, said: String) -> R<String> {
    let md = dir.join("raea/SKILL.md");
    if !dir.join("raea").is_symlink() && let Ok(text) = std::fs::read_to_string(&md) {
        let text = with_preface(&text, dir);
        if text.len() >= MAX_SKILL {
            return Err(format!("{}: {} KB, more than the 256 KB Grok Bot's Add new skill takes", md.display(), text.len() / 1000));
        }
        std::fs::write(&md, text).map_err(|e| format!("{}: {e}", md.display()))?;
    }
    Ok(format!("{said}{}", finish(dir)))
}

/// SKILL.md with the preface after its front matter (or first, when it has none): raea runs on the person's Mac, not on
/// the Bot's computer. A copy that has one keeps only one.
pub fn with_preface(text: &str, dir: &Path) -> String {
    if text.contains("> **On Grok Bot.**") {
        return text.to_string();
    }
    let p = format!(
        "> **On Grok Bot.** raea runs on the person's Mac, not on your computer. Run every `raea` command there, and read\n> every file this skill links (3d.md, reference/cli.md and the rest) from its copy there, in {}, both\n> with Run a command on your local computer (the person turns on Settings → Computer → Execution on this computer).\n> No raea MCP server or hook is set up for you: after saving a composition run `raea check EDIT.tsx --quick` yourself,\n> and `raea show FILE` puts media in front of the person.\n",
        dir.join("raea").display()
    );
    let Some(rest) = text.strip_prefix("---\n") else { return format!("{p}\n{text}") };
    let Some(end) = rest.find("\n---\n") else { return format!("{p}\n{text}") };
    let (front, body) = (&rest[..end], rest[end + 5..].trim_start_matches('\n'));
    format!("---\n{front}\n---\n\n{p}\n{body}")
}

/// A query value as Grok Bot builds its own links (encodeURIComponent): every byte but A-Z a-z 0-9 - _ . ! ~ * ' ( ).
fn encode(v: &str) -> String {
    v.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// One of Grok Bot's deep links (its route table: grokbot://app/v1/ROUTE?KEY=VALUE).
pub fn link(route: &str, key: &str, value: &str) -> String {
    format!("grokbot://app/v1/{route}?{key}={}", encode(value))
}

/// The message to the Bot that makes the skill from the copy staged in `dir`: at most 1000 characters, as its prompt
/// link takes.
fn ask(dir: &Path) -> String {
    let md = dir.join("raea/SKILL.md");
    format!("Please make a private skill named raea from the file {} on my Mac: read it with Run a command on your local computer and keep its text as it is. raea runs on my Mac, so run every raea command there.", md.display())
}

/// What `raea skills install --agent grokbot` says once the copy is staged: where it is, and the links that finish it.
pub fn finish(dir: &Path) -> String {
    let md = dir.join("raea/SKILL.md");
    let here = match app() {
        Some(a) => format!("Grok Bot is at {}.", a.display()),
        None => "Grok Bot isn't installed on this Mac (cursor.com/download/bot); the copy is ready for when it is.".into(),
    };
    format!(
        "{here} It keeps its skills on its own computer and its MCP servers on your account, so the copy waits here: {}. No hooks, and no MCP server: Grok Bot runs a server's command on its computer, where the blender server can't reach this Mac's Blender.\nTo finish, in Grok Bot, signed in:\n  1. turn on Execution on this computer: {}\n  2. send the message this fills in, and the Bot makes the skill from the copy: {}\n     or by hand: Skills → Add new skill → Upload file → {}\n`open LINK` opens a link in Grok Bot",
        dir.display(),
        link("settings", "id", "local-execution"),
        link("prompt", "text", &ask(dir)),
        md.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preface_goes_after_the_front_matter_once() {
        let d = Path::new("/h/.raea/grokbot/skills");
        let md = "---\nname: raea\ndescription: d\nversion: 1\n---\n\n# raea\n";
        let out = with_preface(md, d);
        assert!(out.starts_with("---\nname: raea\ndescription: d\nversion: 1\n---\n\n> **On Grok Bot.** raea runs on the person's Mac"), "{out}");
        assert!(out.contains("in /h/.raea/grokbot/skills/raea, both") && out.ends_with("\n\n# raea\n"), "{out}");
        assert_eq!(with_preface(&out, d), out);
        assert!(with_preface("# no front matter\n", d).starts_with("> **On Grok Bot.**"));
    }

    #[test]
    fn links_are_built_as_grok_bot_builds_its_own() {
        assert_eq!(link("settings", "id", "local-execution"), "grokbot://app/v1/settings?id=local-execution");
        assert_eq!(link("prompt", "text", "a b/ü&+=?#'()"), "grokbot://app/v1/prompt?text=a%20b%2F%C3%BC%26%2B%3D%3F%23'()");
        // its prompt route takes at most 1000 characters of text in a link of at most 2048, with no format characters
        let a = ask(Path::new("/h/a-home-folder-with-a-rather-long-name/.raea/grokbot/skills"));
        assert!(a.chars().count() <= 1000 && link("prompt", "text", &a).len() <= 2048, "{a}");
        assert!(a.chars().all(|c| !c.is_control() && !('\u{200B}'..='\u{200F}').contains(&c) && !('\u{202A}'..='\u{202E}').contains(&c)));
    }

    #[test]
    fn stage_prefaces_the_copy_once_and_refuses_one_too_big_to_upload() {
        let d = std::env::temp_dir().join(format!("raea-grokbot-stage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("raea")).unwrap();
        std::fs::write(d.join("raea/SKILL.md"), "---\nname: raea\nversion: 1\n---\n\n# raea\n").unwrap();
        let out = stage(&d, "copied\n".into()).unwrap();
        assert!(out.starts_with("copied\n") && out.contains("grokbot://app/v1/prompt?text="), "{out}");
        stage(&d, String::new()).unwrap();
        let md = std::fs::read_to_string(d.join("raea/SKILL.md")).unwrap();
        assert_eq!(md.matches("> **On Grok Bot.**").count(), 1, "{md}");
        std::fs::write(d.join("raea/SKILL.md"), "x".repeat(MAX_SKILL)).unwrap();
        assert!(stage(&d, String::new()).unwrap_err().contains("256 KB"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn finish_says_where_the_copy_is_and_both_links() {
        let f = finish(Path::new("/h/.raea/grokbot/skills"));
        assert!(f.contains("the copy waits here: /h/.raea/grokbot/skills."), "{f}");
        assert!(f.contains("grokbot://app/v1/settings?id=local-execution") && f.contains("grokbot://app/v1/prompt?text=Please%20make%20a%20private%20skill"), "{f}");
        assert!(f.contains("Upload file → /h/.raea/grokbot/skills/raea/SKILL.md"), "{f}");
    }
}
