//! `raea app bundle` on the web target: a Mac app that opens the page in raea-page-host's window. App.app/Contents has
//! the page host, the app's script and stylesheet in Resources/, and a launcher in MacOS/ that starts the host on them
//! at the app's title and size, signed by the developer (sign.rs, T1d). The launcher runs the host as its child and
//! waits for it: a launcher that execs it leaves the app without a pid in AppKit (#163, `launcher`).

use std::path::PathBuf;

use crate::verbs::Ask;

fn plist(name: &str, id: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundleDisplayName</key><string>{name}</string>
  <key>CFBundleIdentifier</key><string>{id}</string>
  <key>CFBundleExecutable</key><string>{name}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#,
        name = xml(name)
    )
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A shell word, quoted
fn sh(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// The app's executable (Contents/MacOS/NAME): the page host in Resources, on the app's script and stylesheet, in a
/// window at the app's title and size. The host is this script's child, never exec'd in its place: when the process
/// macOS launched execs another program, AppKit keeps the app's bundle id but loses its pid (NSRunningApplication's
/// processIdentifier -1), so window managers that find an app's windows by its pid never see the window (#163). The
/// host still checks in as the app, from inside it; a signal to the launcher is passed on to it.
fn launcher(name: &str, size: (u32, u32)) -> String {
    format!(
        "#!/bin/sh\n# made by raea app bundle: the page in raea-page-host's window, the host this script's child (an exec loses the app's pid)\nR=\"$(cd \"$(dirname \"$0\")/../Resources\" && pwd)\"\n\"$R/raea-page-host\" \"$R/app.js\" --css \"$R/app.css\" --window --title {} --size {}x{} &\nhost=$!\ntrap 'kill -TERM \"$host\" 2>/dev/null' TERM INT HUP\nwait \"$host\"\n",
        sh(name),
        size.0,
        size.1
    )
}

/// The bundle id from the title: ascii letters and digits, under app.raea.local
fn bundle_id(title: &str) -> String {
    let id: String = title.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    format!("local.raea.app.{}", if id.is_empty() { "app" } else { &id })
}

pub fn web(a: &Ask) -> Result<String, String> {
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from(format!("{}.app", crate::verbs::stem(&a.app))));
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    let contents = out.join("Contents");
    let (macos, res) = (contents.join("MacOS"), contents.join("Resources"));
    for d in [&macos, &res] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    crate::web::build(&a.app, &res)?;
    let host = crate::web::bin()?;
    std::fs::copy(&host, res.join("raea-page-host")).map_err(|e| format!("{}: {e}", host.display()))?;
    let name = &a.title;
    std::fs::write(contents.join("Info.plist"), plist(name, &bundle_id(name))).map_err(|e| e.to_string())?;
    let exe = macos.join(name);
    std::fs::write(&exe, launcher(name, a.size)).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    let mut said = crate::sign::sign(&out, &a.sign)?;
    said.push(format!("{} (web)", out.display()));
    Ok(said.join("\n"))
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    /// The launcher run as macOS runs an app's executable, with a page host that writes down its own pid, its parent's
    /// and its arguments, and exits 3: the host must be the launcher's child (an exec ran it in the launcher's
    /// process, the pid macOS launched, and AppKit lost the app's pid: #163), on the app's files, and the launcher must
    /// end with the host's status.
    #[test]
    fn the_host_runs_as_the_launchers_child() {
        // canonical, so the launcher's own `pwd` names Resources as this test does (macOS's temp folder is a link)
        let dir = dunce::canonicalize(&std::env::temp_dir()).unwrap().join(format!("raea-macapp-launcher-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (macos, res) = (dir.join("T.app/Contents/MacOS"), dir.join("T.app/Contents/Resources"));
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(&res).unwrap();
        let ran = res.join("ran");
        let host = res.join("raea-page-host");
        std::fs::write(&host, format!("#!/bin/sh\necho \"$$ $PPID $*\" > '{}'\nexit 3\n", ran.display())).unwrap();
        let exe = macos.join("T");
        std::fs::write(&exe, super::launcher("My App", (800, 600))).unwrap();
        for f in [&host, &exe] {
            std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut child = Command::new(&exe).spawn().unwrap();
        let launched = child.id().to_string();
        let status = child.wait().unwrap();
        let said = std::fs::read_to_string(&ran).unwrap();
        let mut words = said.split_whitespace();
        let (pid, parent) = (words.next().unwrap(), words.next().unwrap());
        assert_ne!(pid, launched, "the host ran in the launcher's own process (exec'd): {said}");
        assert_eq!(parent, launched, "the host isn't the launcher's child: {said}");
        let args = format!("{0}/app.js --css {0}/app.css --window --title My App --size 800x600", res.display());
        assert_eq!(said.trim_end().splitn(3, ' ').nth(2), Some(args.as_str()));
        assert_eq!(status.code(), Some(3), "the launcher didn't end with the host's status");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
