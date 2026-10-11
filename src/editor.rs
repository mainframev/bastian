use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// The user's editor: `$VISUAL`, then `$EDITOR`, then `vi`, like git.
pub fn command() -> String {
    pick(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok())
}

fn pick(visual: Option<String>, editor: Option<String>) -> String {
    [visual, editor]
        .into_iter()
        .flatten()
        .find(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "vi".to_string())
}

/// Run `cmd` on `path` through the shell, so editor values with arguments
/// (`code --wait`, `nvim -p`) work, opening at `line` (1-based) when the
/// editor is known to support it. Waits for the editor to exit; GUI editors
/// without a wait flag return right away.
pub fn open_with(cmd: &str, path: &Path, line: Option<usize>) -> io::Result<ExitStatus> {
    Command::new("sh")
        .arg("-c")
        .arg(invocation(cmd, line))
        .arg("sh")
        .arg(path)
        .status()
}

/// Shell snippet that opens `"$1"` with `cmd`, at `line` when the editor's
/// syntax for that is known. Unknown editors just get the path.
fn invocation(cmd: &str, line: Option<usize>) -> String {
    let program = cmd
        .split_whitespace()
        .next()
        .and_then(|p| p.rsplit('/').next())
        .unwrap_or("");
    match line {
        Some(n) => match program {
            "vi" | "vim" | "nvim" | "gvim" | "mvim" | "nano" | "emacs" | "emacsclient" | "hx"
            | "helix" | "kak" | "micro" => format!("{cmd} +{n} \"$1\""),
            "code" | "code-insiders" | "codium" | "cursor" | "windsurf" => {
                format!("{cmd} --goto \"$1:{n}\"")
            }
            "zed" | "subl" | "sublime_text" => format!("{cmd} \"$1:{n}\""),
            _ => format!("{cmd} \"$1\""),
        },
        None => format!("{cmd} \"$1\""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_visual_then_editor_then_vi() {
        let s = |v: &str| Some(v.to_string());
        assert_eq!(pick(s("code --wait"), s("vim")), "code --wait");
        assert_eq!(pick(None, s("nvim")), "nvim");
        assert_eq!(pick(s("  "), s("hx")), "hx");
        assert_eq!(pick(None, None), "vi");
    }

    #[test]
    fn opens_known_editors_at_a_line() {
        assert_eq!(invocation("nvim", Some(12)), "nvim +12 \"$1\"");
        assert_eq!(
            invocation("/usr/bin/vim -p", Some(3)),
            "/usr/bin/vim -p +3 \"$1\""
        );
        assert_eq!(
            invocation("code --wait", Some(7)),
            "code --wait --goto \"$1:7\""
        );
        assert_eq!(invocation("zed", Some(9)), "zed \"$1:9\"");
        assert_eq!(invocation("ed", Some(9)), "ed \"$1\"");
        assert_eq!(invocation("nvim", None), "nvim \"$1\"");
    }

    #[test]
    fn line_reaches_the_editor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.md");
        let log = dir.path().join("args");
        std::fs::write(&path, "x\n").unwrap();
        // A fake `vim` that records its arguments.
        let fake = dir.path().join("vim");
        std::fs::write(
            &fake,
            format!("#!/bin/sh\necho \"$@\" > '{}'\n", log.display()),
        )
        .unwrap();
        Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
        open_with(&fake.display().to_string(), &path, Some(42)).unwrap();
        let args = std::fs::read_to_string(&log).unwrap();
        assert_eq!(args.trim(), format!("+42 {}", path.display()));
    }

    #[test]
    fn passes_path_with_spaces_as_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my notes.md");
        std::fs::write(&path, "# x\n").unwrap();
        // A fake "editor" that appends to the file it was given.
        let status = open_with("sh -c 'echo edited >> \"$0\"'", &path, None).unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# x\nedited\n");
    }
}
