use std::path::Path;
use std::sync::mpsc::{self, Sender};

use ignore::WalkState;

use super::Entry;
use crate::config::SortBy;

/// Which files the browser lists, and in what order.
#[derive(Debug, Clone, Default)]
pub struct Scan {
    /// Hidden and gitignored files too.
    pub all: bool,
    pub sort: SortBy,
    /// Extensions of the files listed, in any case, with or without the
    /// dot. Empty means the Markdown ones (`doc::is_markdown_ext`).
    pub extensions: Vec<String>,
    /// Gitignore-style globs of paths to leave out, even with `all`.
    pub ignore: Vec<String>,
}

impl Scan {
    fn lists(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        if self.extensions.is_empty() {
            crate::doc::is_markdown_ext(ext)
        } else {
            self.extensions
                .iter()
                .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(ext))
        }
    }

    /// `ignore` as walk overrides rooted at `root`; bad globs are errors.
    pub fn overrides(&self, root: &Path) -> anyhow::Result<ignore::overrides::Override> {
        let mut b = ignore::overrides::OverrideBuilder::new(root);
        for glob in &self.ignore {
            b.add(&format!("!{glob}"))
                .map_err(|e| anyhow::anyhow!("bad `ignore` glob {glob:?}: {e}"))?;
        }
        Ok(b.build()?)
    }
}

/// Markdown files under `root`, honoring .gitignore and skipping hidden
/// paths unless `opts.all`, in `opts.sort` order.
pub fn discover(root: &Path, opts: &Scan) -> Vec<Entry> {
    let (tx, rx) = mpsc::channel();
    scan(root, opts, &tx);
    // The receiver below ends once every sender is gone.
    drop(tx);
    let mut entries: Vec<Entry> = rx.into_iter().collect();
    sort(&mut entries, opts.sort);
    entries
}

/// Newest first by `modified` or `created` time, or by name A to Z; ties
/// in path order.
pub(super) fn sort(entries: &mut [Entry], by: SortBy) {
    match by {
        SortBy::Name => entries.sort_by(|a, b| {
            a.sort_name
                .cmp(&b.sort_name)
                .then_with(|| a.rel.cmp(&b.rel))
        }),
        SortBy::Modified | SortBy::Created => {
            entries.sort_by(|a, b| b.date(by).cmp(&a.date(by)).then_with(|| a.rel.cmp(&b.rel)));
        }
    }
}

/// Walk `root` on several threads, sending each listed file as it's found.
/// Returns when the walk is done. Hidden and gitignored paths are skipped
/// unless `opts.all` is set, `opts.ignore` matches always are; unreadable
/// entries and broken symlinks are skipped.
pub fn scan(root: &Path, opts: &Scan, tx: &Sender<Entry>) {
    let all = opts.all;
    // Globs were checked at startup.
    let overrides = opts
        .overrides(root)
        .unwrap_or_else(|_| ignore::overrides::Override::empty());
    ignore::WalkBuilder::new(root)
        .overrides(overrides)
        .hidden(!all)
        .git_ignore(!all)
        .git_global(!all)
        .git_exclude(!all)
        .ignore(!all)
        // Hidden paths are skipped here too: a .gitignore `!` rule (e.g.
        // `!/.config/` in a dotfiles repo) would otherwise bring them back.
        // Even with `all`, git's own data is never documentation.
        .filter_entry(move |e| {
            let name = e.file_name();
            name != ".git" && (all || e.depth() == 0 || !name.as_encoded_bytes().starts_with(b"."))
        })
        .build_parallel()
        .run(|| {
            let tx = tx.clone();
            Box::new(move |result| {
                let Ok(d) = result else {
                    return WalkState::Continue;
                };
                if !d.file_type().is_some_and(|t| t.is_file()) || !opts.lists(d.path()) {
                    return WalkState::Continue;
                }
                let path = d.path().to_path_buf();
                let meta = d.metadata().ok();
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                let entry = Entry::new(
                    path.clone(),
                    rel,
                    crate::open::peek_title(&path),
                    meta.as_ref().and_then(|m| m.modified().ok()),
                    meta.as_ref()
                        .and_then(|m| m.created().or_else(|_| m.modified()).ok()),
                );
                if tx.send(entry).is_err() {
                    return WalkState::Quit;
                }
                WalkState::Continue
            })
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn all_files(all: bool) -> Scan {
        Scan {
            all,
            ..Scan::default()
        }
    }

    #[test]
    fn gitignore_exceptions_do_not_show_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".config/app")).unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "/*\n!/.config/\n!/README.md\n").unwrap();
        std::fs::write(root.join("README.md"), "# Readme\n").unwrap();
        std::fs::write(root.join(".config/app/notes.md"), "# notes\n").unwrap();
        std::fs::write(root.join(".hidden.md"), "# hidden\n").unwrap();

        let rels = |all| {
            let mut r: Vec<String> = discover(root, &all_files(all))
                .into_iter()
                .map(|e| e.rel)
                .collect();
            r.sort();
            r
        };
        assert_eq!(rels(false), ["README.md"]);
        assert_eq!(
            rels(true),
            [".config/app/notes.md", ".hidden.md", "README.md"]
        );

        // A hidden directory given as the root is still browsed.
        let inner: Vec<String> = discover(&root.join(".config"), &all_files(false))
            .into_iter()
            .map(|e| e.rel)
            .collect();
        assert_eq!(inner, ["app/notes.md"]);
    }

    #[test]
    fn discovers_markdown_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join("build")).unwrap();
        std::fs::write(root.join("README.md"), "# Readme\n").unwrap();
        std::fs::write(root.join("docs/guide.markdown"), "text\n").unwrap();
        std::fs::write(root.join("build/out.md"), "# ignored\n").unwrap();
        std::fs::write(root.join("notes.txt"), "# not md\n").unwrap();
        std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
        // .gitignore is only honored inside a git repository.
        std::fs::create_dir(root.join(".git")).unwrap();

        let mut rels: Vec<String> = discover(root, &all_files(false))
            .into_iter()
            .map(|e| e.rel)
            .collect();
        rels.sort();
        assert_eq!(rels, ["README.md", "docs/guide.markdown"]);

        // With `all`, hidden and ignored files are listed too.
        std::fs::create_dir_all(root.join(".notes")).unwrap();
        std::fs::write(root.join(".notes/todo.mkd"), "# todo\n").unwrap();
        let mut rels: Vec<String> = discover(root, &all_files(true))
            .into_iter()
            .map(|e| e.rel)
            .collect();
        rels.sort();
        assert_eq!(
            rels,
            [
                ".notes/todo.mkd",
                "README.md",
                "build/out.md",
                "docs/guide.markdown"
            ]
        );
    }

    /// Listed paths under `root` with `opts`, in list order.
    fn listed(root: &Path, opts: &Scan) -> Vec<String> {
        discover(root, opts).into_iter().map(|e| e.rel).collect()
    }

    #[test]
    fn sorts_by_modified_created_or_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Created in this order; modified in the opposite one.
        for (file, title) in [
            ("a.md", "# Zebra\n"),
            ("b.md", "# apple\n"),
            ("c.md", "# Mango\n"),
        ] {
            std::fs::write(root.join(file), title).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let base = SystemTime::now();
        for (i, file) in ["c.md", "b.md", "a.md"].iter().enumerate() {
            let f = std::fs::File::options()
                .write(true)
                .open(root.join(file))
                .unwrap();
            f.set_modified(base + std::time::Duration::from_secs(i as u64 * 60))
                .unwrap();
        }
        let by = |sort| Scan {
            sort,
            ..Scan::default()
        };
        assert_eq!(
            listed(root, &by(SortBy::Modified)),
            ["a.md", "b.md", "c.md"]
        );
        assert_eq!(listed(root, &by(SortBy::Name)), ["b.md", "c.md", "a.md"]);
        // Where the filesystem keeps creation times, newest first.
        let created_kept = std::fs::metadata(root.join("a.md"))
            .unwrap()
            .created()
            .is_ok();
        if created_kept {
            assert_eq!(listed(root, &by(SortBy::Created)), ["c.md", "b.md", "a.md"]);
        }
    }

    #[test]
    fn lists_only_configured_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for f in ["a.md", "b.txt", "c.MDX", "d.rst"] {
            std::fs::write(root.join(f), "x\n").unwrap();
        }
        let mut names = listed(root, &Scan::default());
        names.sort();
        assert_eq!(names, ["a.md", "c.MDX"]);
        let opts = Scan {
            extensions: vec!["txt".into(), ".mdx".into()],
            ..Scan::default()
        };
        let mut names = listed(root, &opts);
        names.sort();
        assert_eq!(names, ["b.txt", "c.MDX"]);
    }

    #[test]
    fn ignore_globs_leave_paths_out_even_with_all() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("drafts/deep")).unwrap();
        std::fs::create_dir_all(root.join(".notes")).unwrap();
        for f in ["keep.md", "drafts/deep/x.md", "CHANGELOG.md", ".notes/n.md"] {
            std::fs::write(root.join(f), "x\n").unwrap();
        }
        let opts = |all| Scan {
            all,
            ignore: vec!["drafts/".into(), "CHANGELOG.md".into()],
            ..Scan::default()
        };
        assert_eq!(listed(root, &opts(false)), ["keep.md"]);
        let mut with_all = listed(root, &opts(true));
        with_all.sort();
        assert_eq!(with_all, [".notes/n.md", "keep.md"]);
    }

    #[test]
    fn bad_ignore_globs_are_errors() {
        let opts = Scan {
            ignore: vec!["a[".into()],
            ..Scan::default()
        };
        let err = opts.overrides(Path::new(".")).unwrap_err().to_string();
        assert!(err.contains("`ignore`") && err.contains("a["), "{err}");
    }
}
