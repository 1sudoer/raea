//! What the person sent from the raea app, handed to the session it's for at its next step (docs/agents.feature A3c;
//! Shane 11-14: a reply in the feed "doesn't confirm, doesn't interrupt the agent", and the app forked a copy of the
//! session so two agents ran). A session that posted with `raea show` has an inbox, ~/.raea/inbox/<inbox>.jsonl, and
//! the app writes there what the person sends on its items or to it in the feed, even while its terminal is busy. This
//! hook reads what was added since it last looked, after any tool call and when a turn is about to end, and hands it
//! over: as the tool's context, or as the reason the turn goes on. How far it has read is kept beside the inbox
//! (<inbox>.read, a byte offset), which the app reads to say the message was read.
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The session's inbox, named as `raea show` names it (engine/cli/src/show.ts posterOf): the harness's name and the
/// first eight of its session id.
pub fn name(agent: &str, v: &Value) -> Option<String> {
    let session = v["session_id"].as_str().map(String::from).or_else(|| std::env::var("CLAUDE_CODE_SESSION_ID").ok()).filter(|s| !s.is_empty())?;
    let slug = match agent {
        "claude" => "claude-code",
        "codex" => "codex",
        "gemini" => "gemini",
        _ => return None,
    };
    // the id's word characters, dots and dashes, as posterOf's /[^\w.-]/ leaves them
    let id: String = session.chars().filter(|c| c.is_ascii_alphanumeric() || "_.-".contains(*c)).take(8).collect();
    Some(format!("{slug}-{id}"))
}

/// $RAEA_INBOX_DIR, else ~/.raea/inbox
pub fn dir() -> PathBuf {
    raea_io_home::env_path("RAEA_INBOX_DIR", ".raea/inbox")
}

/// The lines added to the inbox since the last look, each read once: the cursor moves past them.
pub fn unread(dir: &Path, inbox: &str) -> Vec<Value> {
    let file = dir.join(format!("{inbox}.jsonl"));
    let mark = dir.join(format!("{inbox}.read"));
    let Ok(all) = std::fs::read(&file) else { return vec![] };
    let from = std::fs::read_to_string(&mark).ok().and_then(|s| s.trim().parse::<usize>().ok()).unwrap_or(0).min(all.len());
    // only whole lines: one being written stays for the next look
    let Some(end) = all[from..].iter().rposition(|&b| b == b'\n').map(|i| from + i + 1) else { return vec![] };
    let lines = String::from_utf8_lossy(&all[from..end]).lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let _ = std::fs::write(&mark, end.to_string());
    lines
}

/// One line as the agent reads it: who it's on, what kind, the words.
fn said(l: &Value) -> String {
    let s = |k: &str| l[k].as_str().unwrap_or("").trim().to_string();
    let on = if s("item").is_empty() { String::new() } else { format!(" on item {} \"{}\"", s("item"), s("title")) };
    let kind = s("kind");
    let text = s("text");
    let what = match kind.as_str() {
        "message" => format!("a message{}: {text}", if s("project").is_empty() { String::new() } else { format!(" (project {})", s("project")) }),
        "edits" => format!("edits{on}:\n{text}{}", if s("tasks").is_empty() { String::new() } else { format!("\n({})", s("tasks")) }),
        "note" => format!("a note{on}: {text}"),
        "answer" => format!("answers{on}: {text}"),
        "result" => format!("a result{on}: {text}"),
        "correction" => format!("a correction{on}: {text}"),
        "sign" => format!("a sign-off{on} (ask {})", l["ask"]),
        k => format!("{}{on}: {text}", if k.is_empty() { "something" } else { k }),
    };
    format!("- {what}")
}

/// What the agent is told, or None when nothing's new.
pub fn told(agent: &str, v: &Value) -> Option<String> {
    let inbox = name(agent, v)?;
    let lines = unread(&dir(), &inbox);
    if lines.is_empty() {
        return None;
    }
    let list = lines.iter().map(said).collect::<Vec<_>>().join("\n");
    Some(format!(
        "raea: the person just sent this from the raea app (your inbox, ~/.raea/inbox/{inbox}.jsonl; marked read, so the app shows it as read):\n{list}\nTake it up now, before going on: it may change what you're doing. What you say next shows under your session in their feed; answer an item with raea show --on ITEM --note LINE, or post the next version with raea show FILE --replaces ITEM."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_line_is_handed_over_once_and_a_half_written_one_waits() {
        let d = std::env::temp_dir().join(format!("raea-inbox-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("claude-code-abcd1234.jsonl");
        std::fs::write(&f, "{\"kind\":\"message\",\"text\":\"more epic\"}\n{\"kind\":\"no").unwrap();
        let first = unread(&d, "claude-code-abcd1234");
        assert_eq!(first.len(), 1);
        assert_eq!(said(&first[0]), "- a message: more epic");
        assert!(unread(&d, "claude-code-abcd1234").is_empty());
        std::fs::write(&f, "{\"kind\":\"message\",\"text\":\"more epic\"}\n{\"kind\":\"note\",\"item\":\"i-1\",\"title\":\"Board\",\"text\":\"hills taller\"}\n").unwrap();
        let next = unread(&d, "claude-code-abcd1234");
        assert_eq!(next.len(), 1);
        assert_eq!(said(&next[0]), "- a note on item i-1 \"Board\": hills taller");
        assert_eq!(std::fs::read_to_string(d.join("claude-code-abcd1234.read")).unwrap(), std::fs::metadata(&f).unwrap().len().to_string());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_inbox_is_named_as_raea_show_names_it() {
        assert_eq!(name("claude", &json!({"session_id": "15cd6bb0-8829-466f-a28a-bbca3d35be92"})).as_deref(), Some("claude-code-15cd6bb0"));
        assert_eq!(name("kimi", &json!({"session_id": "x"})), None);
    }
}
