# bastian

Read Markdown in the terminal, and run its code blocks.

bastian is a terminal Markdown reader with one addition: fenced code blocks
in `sh`, `bash`, `zsh`, `fish`, `python` or `node` can be run where you read
them, with their output streamed underneath. READMEs, runbooks and setup guides
become something you can step through instead of copy-paste.

Named after Bastian Balthazar Bux, who reads *The NeverEnding Story* until he
ends up inside it.

## Install

```sh {name=install}
cargo install --path .
```

## Usage

```sh {norun}
bastian                 # browse Markdown files under the current directory
bastian docs/           # browse another directory
bastian -a              # also list hidden and gitignored files
bastian README.md       # read a file
cat notes.md | bastian -
bastian github.com/owner/repo         # a repository's README (GitLab too)
bastian https://example.com/doc.md    # any Markdown URL
bastian https://github.com/o/r/blob/main/docs/guide.md  # file pages are read raw
bastian -p README.md    # print instead of opening the TUI
bastian --pager README.md  # page it through $PAGER (default less -R)
bastian --color always README.md | less -R
bastian -y runbook.md   # run blocks without confirming each one
bastian -n README.md    # show line numbers
bastian -t kanagawa README.md  # use another theme
```

Try it on this file: open it with `bastian README.md`, press `Tab` to focus the
block below and `Enter` to run it.

```sh {name=hello}
echo "hello from $(basename "$PWD")"
uname -sm
```

## Keys

| Key | Reader |
|-----|--------|
| `j/k`, `ctrl-d/u`, `space/b`, `g/G` | scroll |
| `/`, `n/N` | search, next/previous match |
| `Tab`, `Shift-Tab` | focus next/previous code block |
| `Enter`, `r` | run focused block |
| `R` | run all runnable blocks from the focused one, stop at first failure |
| `x` | stop running blocks |
| `o` | show/hide a block's output |
| `y` | copy focused block (OSC 52) |
| `c` | copy the whole document |
| `L` | list links; follow one with `Enter` |
| `e` | edit the file in `$VISUAL`, `$EDITOR` or `vi`, at the line you're reading |
| `ctrl-z` | suspend; resume with `fg` |
| `#` | toggle line numbers |
| `T` | pick a theme (also in the browser) |
| `?` | key help |
| `Esc`, `h` | back to the previous document or the browser |
| `q` | quit |

In the browser: `j/k` to move, `h/l` (or `←/→`) for the previous/next page,
`ctrl-d/u` and `ctrl-f/b` to page, `/` to filter,
`Enter` to open, `e` to edit, `r` to rescan for new or deleted files, `?` for
all keys, `q` to quit.

The search and filter input edits like a shell prompt: `←/→`, `Home/End`
(or `ctrl-a/e`), `alt-b/f` by word, `ctrl-w` and `ctrl-u` to delete. While
filtering, `↑/↓`, `PageUp/PageDown` and `ctrl-j/k` leave the input and move
through the results.

Links are also clickable in terminals that support hyperlinks (OSC 8: iTerm2,
kitty, WezTerm, Ghostty, Windows Terminal, recent tmux with `hyperlinks` in
`terminal-features`, …): web links open as they are, relative links as files
next to the document (or against the URL of a remote document).

The open document reloads when the file changes on disk.

Remote documents (URLs, `github.com/owner/repo`, `gitlab.com/group/project`)
resolve relative links against their URL, so `L` can follow them to other
remote Markdown files. Their code blocks run in the current directory and
always ask first, even with `--yes`. Set `GITHUB_TOKEN` to avoid GitHub's
anonymous rate limit.

`.mdx` files are parsed as MDX: front matter, `import`/`export` and
`{/* comments */}` are hidden, components show as muted tag lines around their
rendered content, and code blocks inside components can still be run. Front
matter is hidden in plain `.md` files too.

## Code block attributes

Put attributes in braces after the language:

````markdown
```sh {name=deploy cwd=./infra env=STAGE=prod timeout=5m confirm}
./deploy.sh
```
````

| Attribute | Effect |
|-----------|--------|
| `name=...` | label shown on the block and in prompts; quote values with spaces |
| `cwd=...` | working directory, relative to the document's directory |
| `env=KEY=VALUE` | environment variable; repeat for more |
| `timeout=30s` | stop the block after `ms`, `s`, `m` or `h` |
| `confirm` | always ask before running, even with `--yes` |
| `norun` | never run this block |

Blocks run in the document's directory with no terminal attached, so
interactive programs (editors, ssh, password prompts) don't work. Each block
runs in its own process group; stopping it or quitting bastian stops everything
it started.

## Configuration

`~/.config/bastian/config.toml` (or `$XDG_CONFIG_HOME/bastian/config.toml`).
Command-line flags take precedence.

```toml
theme = "auto"        # match the terminal background, "dark", "light" or a theme name
dark_theme = "rose-pine"        # used by "auto" on dark backgrounds, and "dark"
light_theme = "rose-pine-dawn"  # used by "auto" on light backgrounds, and "light"
width = 100           # maximum document width
confirm = "always"    # "never" behaves like --yes
color = "auto"        # printed output: "auto", "always" or "never"
pager = false         # true: show documents in $PAGER instead of the TUI
all = false           # true: also list hidden and gitignored files
mouse = false         # true: scroll with the mouse wheel
logo = true           # false: no logo at the top of the file browser (--no-logo)
line_numbers = false  # true behaves like --line-numbers

[interpreters]
# The block source is appended as the last argument.
ruby = ["ruby", "-e"]
python = ["uv", "run", "python", "-c"]
fish = []             # an empty list stops a language from running
```

Printed output is colored when it goes to a terminal; `NO_COLOR` turns that
off and `CLICOLOR_FORCE=1` turns it on for pipes. Without `-w` it uses the
terminal width (up to 120 columns), or 80 when there's no terminal.

Use another file with `--config <path>` or `BASTIAN_CONFIG` (`~` and `$VAR` are
expanded). `bastian config` opens the file in your editor, creating it from a
commented template first, and `bastian --print-config` shows the effective
settings and where each one came from. To read a file literally named
`config`, write `./config`.

Each setting can also come from the environment: `BASTIAN_THEME`,
`BASTIAN_DARK_THEME`, `BASTIAN_LIGHT_THEME`, `BASTIAN_WIDTH`, `BASTIAN_CONFIRM`, `BASTIAN_LINE_NUMBERS`, `BASTIAN_COLOR`,
`BASTIAN_PAGER`, `BASTIAN_ALL`, `BASTIAN_MOUSE` and `BASTIAN_LOGO`. Precedence is
defaults < config file < environment < command-line flags.

## Themes

Built-in themes, which also color code blocks:

| Family | Themes |
|---|---|
| Tokyo Night | `tokyonight-night` (`tokyonight`), `tokyonight-storm`, `tokyonight-moon`, `tokyonight-day` |
| Rosé Pine | `rose-pine`, `rose-pine-moon`, `rose-pine-dawn` |
| Kanagawa | `kanagawa-wave` (`kanagawa`), `kanagawa-dragon`, `kanagawa-lotus` |
| Catppuccin | `catppuccin-mocha` (`catppuccin`), `catppuccin-macchiato`, `catppuccin-frappe`, `catppuccin-latte` |
| Gruvbox | `gruvbox-dark` (`gruvbox`), `gruvbox-light` |
| Nord | `nord` |
| Dracula | `dracula` |
| Everforest | `everforest-dark` (`everforest`), `everforest-light` |
| Solarized | `solarized-dark` (`solarized`), `solarized-light` |

Pick one with `-t`, `theme = "..."` or `BASTIAN_THEME`. With `theme = "auto"`
(the default), bastian asks the terminal for its background color and uses
`dark_theme` or `light_theme`; `"dark"` and `"light"` force one of the two.
`bastian themes` opens a theme picker over a sample document: move through the
list to preview each theme, `Enter` saves the highlighted one as `theme` in
your config file (other settings and comments are kept), `Esc` leaves things
as they were. Press `T` for the same picker while reading or browsing. Piped,
`bastian themes` prints the list with each theme's colors instead.

### Your own themes

Put `<name>.toml` in `~/.config/bastian/themes/` (the `themes` directory next
to the config file) and select it by name. A theme can start from another and
change only some colors:

```toml
# ~/.config/bastian/themes/midnight.toml
extends = "tokyonight-night"

[ui]
accent = "#ff9e64"
h1_bg = "#ff9e64"

[syntax]
comment = "#7a88cf"
```

Without `extends`, a theme sets `appearance` (`"dark"` or `"light"`) and every
color. `[ui]` colors the document: `text`, `muted`, `subtle`, `surface`,
`accent`, `h1_fg`, `h1_bg`, `heading`, `link`, `code`, `match_bg`,
`current_bg`, `err`, `ok`, `warn`. `[syntax]` colors code blocks: `comment`,
`keyword`, `string`, `number`, `constant`, `function`, `type`, `operator`,
`punctuation`, `tag`, `attribute`, `inserted`, `deleted`. The built-in themes
in the repository's `themes/` directory are complete examples. A file named
after a built-in replaces it; to tweak a built-in in place, extend it under its
own name (`extends = "nord"` in `themes/nord.toml`).

## Shell completions and man page

```sh {norun}
bastian completions zsh > ~/.zfunc/_bastian   # also bash, fish, elvish, powershell
bastian man > bastian.1 && man ./bastian.1
```

## Development

```sh {name=check}
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
