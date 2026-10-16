# The reader

Opening a document, from the [file list](file-list.md) or with
`bastian README.md`, shows it laid out for the terminal: headings, lists and
tables fitted to the width, highlighted code, links, images, quotes and
GitHub alerts (`> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`,
`[!CAUTION]`) and footnotes, which are numbered and listed at the end. A
footnote reference is a link: use `L` to list it, then `Enter` to jump to
its note.

← [README](../README.md)

## Keys

| Key                         | Action                                                       |
| --------------------------- | ------------------------------------------------------------ |
| `j/k`, `↓/↑`                | scroll a line                                                |
| `ctrl-d/u`                  | scroll half a page                                           |
| `space/b`, `PageDown/Up`    | scroll a page                                                |
| `g/G`, `Home/End`           | top/bottom                                                   |
| `:`                         | go to a line: type its number, `Enter`                       |
| `/`, `n/N`                  | search, next/previous match                                  |
| `t`                         | show/hide the table of contents                              |
| `J/K`                       | jump to the next/previous heading                            |
| `1-9`, `0`                  | table of contents open: jump to entry 1–9, 10; again: within |
| `<`, `>`                    | table of contents open: narrow/widen it                      |
| `L`                         | list links; `Enter` follows one                              |
| `s/S`                       | focus the next/previous link; `Enter` follows it             |
| `Tab`, `Shift-Tab`          | focus the next/previous code block                           |
| `Enter`, `r`                | run the focused block                                        |
| `R`                         | run every block from the focused one, stopping at a failure  |
| `x`                         | stop running blocks                                          |
| `o`                         | show/hide a block's output                                   |
| `y`                         | copy the focused block                                       |
| `d`, `D`                    | focus the next/previous details block (`Enter` opens/closes) |
| `v`                         | show the focused diagram as source or drawing                |
| `c`                         | copy the whole document                                      |
| `p`                         | show the file path; `R`/`A` copy it relative/absolute        |
| `ctrl-e`                    | edit the file at the line you're reading                     |
| `#`                         | toggle line numbers                                          |
| `i`                         | show/hide images                                             |
| `T`                         | pick a theme                                                 |
| `H`                         | list the documents opened this session; `Enter` reopens one  |
| `ctrl-z`                    | suspend; resume with `fg`                                    |
| `?`                         | key help                                                     |
| `Esc`                       | clear focus and search; pressed again, back                 |
| `h`                         | back to the previous document or the file list              |
| `q`                         | quit                                                         |

`--mouse` (or `mouse = true`) also scrolls with the mouse wheel, and a click
on the `<` or `>` in the header of the table of contents or the theme picker
resizes it. It captures
the mouse, so selecting text in the terminal then needs a modifier key (often
shift or option).

## Search

`/` searches the rendered text as you type; `Enter` keeps the search, `n` and
`N` move between matches and `Esc` clears it along with any focus. The input
edits like a shell
prompt (`←/→`, `alt-b/f`, `ctrl-w`, `ctrl-u`, …).

## Table of contents

`t` shows the document's headings in a pane on the left. The document
narrows and re-wraps beside it, and the usual keys keep working, so you can
read with it open. The section you're reading is highlighted as you scroll.

The outermost level with more than one heading is numbered `01`, `02`, …, so
a single title leaves the numbers to its sections; a document where no level
has two headings has no numbers. Deeper headings are indented under the
numbered ones and numbered within their section, and any above them indent
by their depth. `1` to `9` jump to the numbered headings 1–9 and `0` to the
tenth. The same number again walks the section it jumped to, heading by
heading in document order: `1 1 1 1 1` goes A, A1, A1a, A2, A2a, and past the
section's last heading back to A. Another number right after goes into the
heading reached: `2` then `1` is the first heading under the second. When that
heading has no heading of that number, or once the view has moved, numbers
count from the outermost level again. `J` and `K` step to the next or previous
heading of any level, with the pane open or not, which reaches every heading
in longer documents.

`<` and `>` narrow or widen the pane by four columns, from 16 columns up to
half the screen; with `--mouse`, clicking the `<` or `>` in its header does
the same. The width lasts until bastian quits. Screens narrower than 40
columns leave the pane out.

The theme picker (`T`) opens as a pane on the right, and the document
re-wraps between the two. When the screen is too narrow for both, the table
of contents is hidden until the picker closes; see [themes](themes.md).

`t` replaces the outline that `O` opened in earlier versions.

## Links

`L` lists the document's links. Move with `j`/`k`, `g`/`G`, `ctrl-d`/`ctrl-u`
(half a list) and `PgDn`/`PgUp` (a list), or press `/` and type to keep only
links whose text or URL contains it; `Esc` clears the filter, then closes the
list. `Enter` follows the selected one:

- `#anchor` links scroll to their heading;
- links to Markdown files open in bastian (`Esc` comes back);
- anything else goes to your system's opener (a browser for web links).
  Only `http`, `https` and `mailto` links open straight away; other schemes,
  `file:` URLs and local files show the target and wait for `y`/`n`. A
  target too long for the screen keeps its start and end, with `…` between.

Wiki links work too: `[[Page]]` opens `Page.md` next to the document,
`[[notes/Page|text]]` shows "text" and opens `notes/Page.md`, and
`[[Page#Heading]]` or `[[#Heading]]` scroll to that heading. Pages are found
by their path from the document, not searched for across folders.

`s` and `S` step through the links in place instead, starting from the ones
in view. The status bar shows where the focused link leads before you follow
it: the heading an anchor points to, a footnote's text, a Markdown file's
title or the URL. `Enter` follows it and `Esc` lets go.

Links are also clickable in terminals that support hyperlinks (OSC 8: iTerm2,
kitty, WezTerm, Ghostty, Windows Terminal, recent tmux with `hyperlinks` in
`terminal-features`, …). Since a click skips the prompt, only web and mail
links and links to local Markdown files are made clickable; follow the rest
with `L`. Set `hyperlinks = false` if your terminal prints the escape codes
instead.

## Images

An image on a line of its own is drawn as a picture: kitty graphics in kitty
and Ghostty, iTerm2's protocol in iTerm2 and WezTerm, and colored half-blocks
elsewhere and inside tmux. It can come from a file next to the document or a
URL. Images in running text, SVGs (such as most badges), GIFs and printed
output keep their alt text.

`i` turns pictures on and off for the session. `images = false` (or
`--images off`) starts with them off; `image_height` caps how many rows a
picture takes (24 by default).

## Details

HTML `<details>` blocks start closed, showing their `<summary>` (or
"Details") after a `▶`; `<details open>` starts open. `d` and `D` focus the
next and previous one and `Enter` opens or closes it. Markdown inside is
rendered when it's separated from the tags by blank lines, as on GitHub.
Search only finds text in open blocks. Printed output shows every block
open.

## Diagrams

Built with the `mermaid` feature (`cargo install --locked bastian --features
mermaid`), ` ```mermaid ` blocks are drawn as box-drawing diagrams, in the
reader and in printed output. Flowcharts, sequence, state, class and ER
diagrams are supported. A diagram that doesn't parse, is another type, is
wider than the window or takes over 250 ms to lay out shows its source
instead, and `v` switches the focused block between the two. The feature is
off by default as it adds about 8 MB to the binary.

## History

`H` lists the documents read this session, most recent first, with the one
you're reading marked and the one before it selected. `Enter` opens the
selection, and `Esc` goes back from it as from a followed link. The list isn't
kept after bastian exits.

## Line numbers

`#` toggles line numbers (or start with `-n`). Source files opened directly,
such as `bastian main.rs`, always show them.

`:` asks for a line number and scrolls that line to the top: the number the
line numbers show, so a source line in source files. A number past the end goes
to the last line. The line is highlighted, with every row it wraps onto, until
the next key.

## Editing and live reload

`ctrl-e` opens the file in your editor at the line you're reading. The editor is
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

`y` copies the focused code block and `c` the whole document's source. `p`
shows where the file is, relative to the current directory and in full; `R`
and `A` copy one of the two (for a remote document, its URL). All of these
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
