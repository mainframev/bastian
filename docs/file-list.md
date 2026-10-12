# The file list

Run `bastian` with no arguments, or with a directory, to list the Markdown in
it and its subdirectories.

← [README](../README.md)

```sh {norun}
bastian            # the current directory
bastian docs/      # another directory
bastian -a         # also hidden and gitignored files
```

Files are listed newest first, each with its title (front matter `title:` or
the first `# ` heading), its path and its date. Hidden paths, `.git` and
anything your `.gitignore` files leave out are skipped unless you pass `-a`
(or set `all = true`). The list fills in while the scan runs; `r` scans again
for new or deleted files.

## Keys

| Key                     | Action                     |
| ----------------------- | -------------------------- |
| `j/k`, `↓/↑`            | move                       |
| `h/l`, `←/→`            | previous/next page         |
| `ctrl-d/u`              | half a page                |
| `ctrl-f/b`              | a full page                |
| `g/G`                   | first/last file            |
| `/`                     | filter                     |
| `Enter`                 | open                       |
| `e`                     | edit in your editor        |
| `r`                     | rescan for files           |
| `T`                     | pick a theme               |
| `Esc`                   | clear the filter, or quit  |
| `?`                     | key help                   |
| `q`                     | quit                       |

## Filtering

`/` opens the filter; typing matches against each file's path and title,
fuzzily, best matches first. `Enter` keeps the filter and returns to the list,
`Esc` clears it.

The filter input edits like a shell prompt: `←/→`, `Home/End` (or
`ctrl-a/e`), `alt-b/f` by word, `ctrl-w` and `ctrl-u` to delete. While typing,
`↑/↓`, `PageUp/PageDown` and `ctrl-j/k` leave the input and move through the
results.

## Shaping the list

All of these go in the [config file](configuration.md); some also have flags.

```toml
sort = "modified"        # "modified", "created" or "name"         (--sort)
selector = "bar"         # "bar", "thick", "block", "arrow" or "highlight"  (--selector)
list_layout = "cards"    # "cards" or "compact", one file per row  (--list-layout)
dates = true             # false hides the dates
date_format = "%d %b %Y %H:%M %Z"   # strftime format of the dates
extensions = ["md", "markdown", "mdx", "mdown", "mkdn", "mkd"]
ignore = ["drafts/", "CHANGELOG.md"]  # gitignore-style globs, even with -a
logo = true              # false hides the logo (--no-logo)
```

- `sort = "created"` shows creation dates; the other orders show modification
  dates.
- `selector` sets how the selected file is marked: a thin bar (the default), a
  thick bar, a block, an arrow beside its title, or a highlighted background.
- `list_layout = "compact"` fits a file per row (title, path and date), so a
  page holds many more.
- `extensions` decides which files are listed, so you can browse `.txt` notes
  too; opening a file directly always works.
- An invalid `ignore` glob or `date_format` stops bastian with an error naming
  the setting.
