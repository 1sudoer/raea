//! Where each agent `raea skills install` knows reads a person's skills, one folder per skill holding SKILL.md. Checked
//! by hand on 11-05: claude: ~/.claude/skills (Claude Code's docs); codex: ~/.agents/skills
//! (developers.openai.com/codex/skills, the user scope; older builds read ~/.codex/skills); kimi: $KIMI_CODE_HOME/skills,
//! else ~/.kimi-code/skills (kimi 2.3.3's own binary; it also reads ~/.agents/skills and ~/.claude/skills); gemini:
//! ~/.gemini/skills (geminicli.com/docs/cli/skills). grokbot (11-14): Grok Bot reads no folder here, so its copy is
//! staged in raea's own (agent_grokbot.rs).
use std::path::PathBuf;

/// The agents, in the order `raea doctor` and the usage name them.
pub const AGENTS: [&str; 5] = ["claude", "codex", "kimi", "gemini", "grokbot"];

/// The folder `agent` reads skills from.
pub fn folder(agent: &str) -> Result<PathBuf, String> {
    let home = crate::agent_mcp::user_home()?;
    Ok(match agent {
        "claude" => home.join(".claude/skills"),
        "codex" => home.join(".agents/skills"),
        "kimi" => std::env::var_os("KIMI_CODE_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".kimi-code")).join("skills"),
        "gemini" => home.join(".gemini/skills"),
        "grokbot" => crate::agent_grokbot::skills_dir()?,
        a => return Err(format!("no agent {a:?}: claude, codex, kimi, gemini or grokbot")),
    })
}

/// Each agent and the folder it reads skills from, for `raea doctor` (an agent with no folder is skipped).
pub fn agents() -> Vec<(&'static str, PathBuf)> {
    AGENTS.into_iter().filter_map(|a| folder(a).ok().map(|t| (a, t))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_agent_has_its_folder() {
        assert!(folder("claude").unwrap().ends_with(".claude/skills"));
        assert!(folder("codex").unwrap().ends_with(".agents/skills"));
        assert!(folder("gemini").unwrap().ends_with(".gemini/skills"));
        assert_eq!(folder("grokbot").unwrap(), crate::agent_grokbot::skills_dir().unwrap());
        assert!(folder("cursor").is_err());
        assert_eq!(agents().len(), AGENTS.len());
    }
}
