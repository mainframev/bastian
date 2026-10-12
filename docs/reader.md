# The reader

Opening a document, from the [file list](file-list.md) or with
`bastian README.md`, shows it laid out for the terminal: headings, lists and
tables fitted to the width, highlighted code, links, images and quotes.

← [README](../README.md)

## Keys

| Key                         | Action                                                       |
| --------------------------- | ------------------------------------------------------------ |
| `j/k`, `↓/↑`                | scroll a line                                                |
| `ctrl-d/u`                  | scroll half a page                                           |
| `space/b`, `PageDown/Up`    | scroll a page                                                |
| `g/G`, `Home/End`           | top/bottom                                                   |
| `/`, `n/N`                  | search, next/previous match                                  |
| `O`                         | outline: jump to a heading                                   |
| `L`                         | list links; `Enter` follows one                              |
| `Tab`, `Shift-Tab`          | focus the next/previous code block                           |
| `Enter`, `r`                | run the focused block                                        |
| `R`                         | run every block from the focused one, stopping at a failure  |
| `x`                         | stop running blocks                                          |
| `o`                         | show/hide a block's output                                   |
| `y`                         | copy the focused block                                       |
| `c`                         | copy the whole document                                      |
| `e`                         | edit the file at the line you're reading                     |
| `#`                         | toggle line numbers                                          |
| `i`                         | show/hide images                                             |
| `T`                         | pick a theme                                                 |
| `ctrl-z`                    | suspend; resume with `fg`                                    |
| `?`                         | key help                                                     |
| `Esc`, `h`                  | back to the previous document or the file list              |
| `q`                         | quit                                                         |

`--mouse` (or `mouse = true`) also scrolls with the mouse wheel. It captures
the mouse, so selecting text in the terminal then needs a modifier key (often
shift or option).

## Search

`/` searches the rendered text as you type; `Enter` keeps the search, `n` and
`N` move between matches and `Esc` clears it. The input edits like a shell
prompt (`←/→`, `alt-b/f`, `ctrl-w`, `ctrl-u`, …).

## Outline

`O` docks the document's headings on the right as a tree. Move with `j/k`,
press `Enter` to jump to a heading, `Esc` to close it.

## Links

`L` lists the document's links. `Enter` follows the selected one:

- `#anchor` links scroll to their heading;
- links to Markdown files open in bastian (`Esc` comes back);
- anything else goes to your system's opener (a browser for web links).

Links are also clickable in terminals that support hyperlinks (OSC 8: iTerm2,
kitty, WezTerm, Ghostty, Windows Terminal, recent tmux with `hyperlinks` in
`terminal-features`, …). Set `hyperlinks = false` if your terminal prints
the escape codes instead.

## Images

An image on a line of its own is drawn as a picture: kitty graphics in kitty
and Ghostty, iTerm2's protocol in iTerm2 and WezTerm, and colored half-blocks
elsewhere and inside tmux. It can come from a file next to the document or a
URL. Images in running text, SVGs (such as most badges), GIFs and printed
output keep their alt text.

`i` turns pictures on and off for the session. `images = false` (or
`--no-images`) starts with them off; `image_height` caps how many rows a
picture takes (24 by default).

## Line numbers

`#` toggles line numbers (or start with `-n`). Source files opened directly,
such as `bastian main.rs`, always show them.

## Editing and live reload

`e` opens the file in your editor at the line you're reading. The editor is
the `editor` setting, then `$VISUAL`, then `$EDITOR`, then `vi`; known editors
(vim, Neovim, Helix, VS Code, …) open at the right line. The open document
reloads whenever the file changes on disk, so edits show up as soon as you
save.

## Reading position

Leaving a file (going back, following a link or quitting) remembers the
section you were reading, and opening it again scrolls back there, even at a
different width. Positions live in `$XDG_STATE_HOME/bastian/positions.json`
(or `~/.local/state/bastian/positions.json`) and are forgotten after 90 days. Set
`remember_position = false` to turn this off.

## Copying

`y` copies the focused code block and `c` the whole document's source. Both
use OSC 52, so they work over ssh in terminals that support it.

## Remote documents

URLs, `github.com/owner/repo` and `gitlab.com/group/project` open a single
document (for a repository, its README). GitHub and GitLab file pages are read
as raw files. Relative links resolve against the document's URL, so `L` can
follow them to other remote Markdown files. Code blocks in remote documents
run in the current directory and always ask first, even with `--yes`. Set
`GITHUB_TOKEN` to avoid GitHub's anonymous rate limit.

## MDX and front matter

`.mdx` files are parsed as MDX: front matter, `import`/`export` and
`{/* comments */}` are hidden, components show as muted tag lines around their
rendered content, and code blocks inside components can still be run. Front
matter is hidden in plain `.md` files too.
