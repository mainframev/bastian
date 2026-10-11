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
/// (`code --wait`, `nvim -p`) work. Waits for the editor to exit; GUI
/// editors without a wait flag return right away.
pub fn open_with(cmd: &str, path: &Path) -> io::Result<ExitStatus> {
    Command::new("sh")
        .arg("-c")
        .arg(format!("{cmd} \"$1\""))
        .arg("sh")
        .arg(path)
        .status()
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
    fn passes_path_with_spaces_as_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my notes.md");
        std::fs::write(&path, "# x\n").unwrap();
        // A fake "editor" that appends to the file it was given.
        let status = open_with("sh -c 'echo edited >> \"$0\"'", &path).unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# x\nedited\n");
    }
}
