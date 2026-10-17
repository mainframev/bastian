# The reader

Opening a document, from the [file list](file-list.md) or with
`bastian README.md`, shows it laid out for the terminal: headings, lists and
tables fitted to the width, highlighted code, task lists, links, images,
quotes and GitHub alerts (`> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`,
`[!WARNING]`, `[!CAUTION]`) and footnotes, which are numbered and listed at
the end. A
footnote reference is a link: use `L` to list it, then `Enter` to jump to
its note.

![Tables, alerts, tasks, highlights, math, footnotes and a details block](https://upnsbzkykttnbdp1.public.blob.vercel-storage.com/demo/markdown.gif)

← [README](../README.md)

## Keys

| Key                         | Action                                                       |
| --------------------------- | ------------------------------------------------------------ |
| `j/k`, `↓/↑`                | scroll a line; table of contents open: next/previous heading |
| `ctrl-d/u`                  | scroll half a page                                           |
| `space/b`, `PageDown/Up`    | scroll a page                                                |
| `g/G`, `Home/End`           | top/bottom                                                   |
| `:`, `ctrl-g`               | go to a line: type its number, `Enter`                       |
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

## Mouse

bastian captures the mouse, so the wheel and the trackpad scroll the
document, `wheel_lines` lines per step (3 by default), and move through the
file list a file per step. A click on the `<` or `>` in the header of
the table of contents or the theme picker resizes it. The link under the
pointer is shaded, so it's clear what a click on it (which most terminals
open with a modifier key) will follow.

While the mouse is captured, selecting text needs a modifier key in most
terminals: hold shift while dragging (most Linux terminals, kitty, WezTerm,
Alacritty), or option/alt (iTerm2, macOS Terminal).

`--no-mouse` (or `mouse = false`, `BASTIAN_MOUSE=false`) leaves the mouse to
the terminal: plain dragging selects text, and the terminal turns the wheel
into arrow keys, which scroll a line at a time.

## Search

`/` searches the rendered text as you type; `Enter` keeps the search, `n` and
`N` move between matches and `Esc` clears it along with any focus. The input
edits like a shell
prompt (`←/→`, `alt-b/f`, `ctrl-w`, `ctrl-u`, …).

![Searching, line numbers and going to a line](https://upnsbzkykttnbdp1.public.blob.vercel-storage.com/demo/search.gif)

## Table of contents

`t` shows the document's headings in a pane on the left. The document
narrows and re-wraps beside it, and the usual keys keep working, so you can
read with it open, except that `j`/`k` (and `↓`/`↑`) step to the next or
previous heading instead of scrolling a line; `ctrl-d/u` and `space/b` still
scroll. The section you're reading is highlighted as you scroll.

![The table of contents: jumping to sections and walking into one](https://upnsbzkykttnbdp1.public.blob.vercel-storage.com/demo/toc.gif)

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
half the screen; clicking the `<` or `>` in its header does the same
(unless the mouse is off). The width lasts until bastian quits. Screens narrower than 40
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

![Following links, wiki links and the history list](https://upnsbzkykttnbdp1.public.blob.vercel-storage.com/demo/links.gif)

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

## Inline HTML and highlights

Paired inline formatting tags style their text, as a browser would: `<b>`
and `<strong>` bold, `<i>` and `<em>` italic, `<u>` underlined, `<s>`,
`<del>` and `<strike>` struck through, `<code>` and `<kbd>` as inline code,
and `<mark>` highlighted. `<br>` (or `<br/>`) breaks the line. Tag names
match in any case and attributes are ignored. `==text==` is highlighted
too, except inside code; a lone `==`, or one with a space on its inner side
(`a == b`), stays as text. Highlights take the theme's warning colour,
since backgrounds already mark search matches and code.

HTML comments (`<!-- prettier-ignore -->` and the like) aren't shown. Other
HTML, and a tag without its partner, shows as its source text.

In `.mdx` files the same lowercase elements are styled the same way;
capitalized ones are components and keep their muted tag labels. JSX needs
`<br />` there: a bare `<br>` is invalid MDX, so the file is read as plain
Markdown.

## Diagrams

Built with the `mermaid` feature (`cargo install --locked bastian --features
mermaid`), ` ```mermaid ` blocks are drawn as box-drawing diagrams, in the
reader and in printed output. Flowcharts, sequence, state, class and ER
diagrams are supported. A diagram that doesn't parse, is another type, is
wider than the window or takes over 250 ms to lay out shows its source
instead, and `v` switches the focused block between the two. The feature is
off by default as it adds about 8 MB to the binary.

![Mermaid flowchart and sequence diagrams](https://upnsbzkykttnbdp1.public.blob.vercel-storage.com/demo/mermaid.gif)

## Math

TeX math reads as Unicode text: `$\alpha^2 \leq \frac{a+b}{2}$` shows as
`α² ≤ (a+b)/2`, in italics like emphasis. `$$…$$` and ` ```math ` blocks are
display math, centered on lines of their own (`\\` starts a new row).
Greek letters, common operators, relations and arrows become their symbols;
superscripts and subscripts use Unicode's small digits and letters where they
exist and fall back to `^(…)` and `_(…)`; `\frac{a}{b}` is `a/b` and
`\sqrt{x}` is `√x`, parenthesizing parts of more than one term. `\left`,
`\right`, `\mathrm`, `\text` and the like are dropped, leaving their
content. Commands bastian doesn't know stay as their source text. A single `$`
with a space just inside it, as in `$5 and $10`, is a dollar sign. ` ```latex `
and ` ```tex ` blocks stay code.

## History

`H` lists the documents read this session, most recent first, with the one
you're reading marked and the one before it selected. `Enter` opens the
selection, and `Esc` goes back from it as from a followed link. The list isn't
kept after bastian exits. `history = false` in the config turns the list off;
`Esc` still goes back from followed links.

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

`.mdx` files are parsed as MDX: `import`/`export` and `{/* comments */}` are
hidden, components show as muted tag lines around their rendered content, and
code blocks inside components can still be run.

YAML (`---`) and TOML (`+++`) front matter, in `.md` and `.mdx` files alike,
shows at the top of the document as a table of its keys and values:

```text {norun}
 Key             │ Value
─────────────────┼─────────────────────
 title           │ Release notes
 tags            │ rust, cli
 seo.description │ What changed in 0.4
```

Lists are joined with commas, nested keys are written `parent.child`, and the
tables of a TOML array of tables show as `{key: value}`. Front matter that can't be read
this way, such as YAML with a list of maps, shows as its source in a code block
instead. `front_matter = "hide"` (or `--front-matter hide`,
`BASTIAN_FRONT_MATTER=hide`) leaves it out. Either way, a `title:` in it still
names the file in the [file list](file-list.md).
