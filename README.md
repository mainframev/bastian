# bastian

![bastian: a terminal Markdown reader](assets/bastian.png)

[![Latest release](https://img.shields.io/crates/v/bastian.svg)](https://crates.io/crates/bastian)
[![Downloads](https://img.shields.io/crates/d/bastian.svg)](https://crates.io/crates/bastian)
[![Build status](https://github.com/mainframev/bastian/actions/workflows/ci.yml/badge.svg)](https://github.com/mainframev/bastian/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> [!WARNING]
> bastian is pre-1.0 and still changing: settings, flags and keys may change
> between minor releases. Check the release notes when you update.

**Make documentation a pleasure to read, right in your terminal.**

bastian finds the Markdown in a folder and lays it out for the terminal:
fitted tables, highlighted code, clickable links, images, a table of
contents, search and live reload. It also reads single documents from a
file, stdin, a URL or a GitHub/GitLab repository, and can run a runbook's
code blocks in place.

![Browsing, filtering, searching and the outline in bastian](demo/usage.gif)

## Install

```sh {norun}
brew install mainframev/tap/bastian      # Homebrew, macOS or Linux
```

```sh {name=install}
cargo install --locked bastian
```

```sh {norun}
cargo binstall bastian                   # prebuilt binary, no compiling
```

Homebrew also installs the shell completions. With Cargo
([rustup](https://rustup.rs)), the latest unreleased version is
`cargo install --locked --git https://github.com/mainframev/bastian`. Add
`--features mermaid` to draw Mermaid diagrams (see
[diagrams](docs/reader.md#diagrams)).

## The TUI

Run `bastian` in a folder (or `bastian docs/`) to see its Markdown files,
newest first:

- `j/k` to move, `/` to filter by path or title, `Enter` to open, `e` to edit;
- hidden and gitignored files are skipped unless you pass `-a`;
- choose how the list looks and sorts: a compact one-row layout, several
  selection styles, sorting by modified or created date or by name.

In a document:

- `/` searches, `n/N` jump between matches;
- `t` shows a table of contents beside the document; `1`–`9` and `0` jump
  to its sections and `J/K` step through headings;
- `L` lists the links to follow (`[[wiki links]]` included), and links are
  clickable in terminals that support it;
- images are drawn as pictures where the terminal can; `i` toggles them;
- `<details>` blocks fold: `d` focuses one and `Enter` opens or closes it;
- `#` shows line numbers, `y`/`c` copy a code block or the whole document,
  `p` shows and copies the file path;
- `ctrl-e` opens your editor at the line you're reading, and the document reloads
  when the file changes;
- `T` switches the theme;
- `?` shows every key, `Esc` goes back, `q` quits.

![Switching themes with T while reading](demo/reader-themes.gif)

More in [the file list](docs/file-list.md) and [the reader](docs/reader.md).

## The CLI

bastian also reads documents from anywhere, and prints them for pipes and
pagers:

```sh {norun}
bastian README.md                    # read a file in the TUI
bastian -                            # read stdin
bastian https://example.com/doc.md   # a Markdown URL
bastian github.com/owner/repo        # a repository's README (GitLab too)
bastian -p README.md                 # print it instead
bastian -p -w 80 README.md           # at most 80 columns wide
bastian --pager README.md            # page it through $PAGER
bastian --color always README.md | less -R
```

MDX files are supported, and source files are shown highlighted. Every input
form, option and command is in [the command line](docs/cli.md).

## Running code blocks

Shell, Python and Node blocks can run right in the reader, with their output
underneath. Try it here: open this file with `bastian README.md`, press `Tab`
to focus the block below and `Enter` to run it.

```sh {name=hello}
echo "hello from $(basename "$PWD")"
uname -sm
```

Attributes like `{cwd=./infra timeout=5m confirm}` control how a block runs;
see [code blocks](docs/code-blocks.md).

## Themes

Built in: `terminal` (your terminal's own colors), neutral dark and light,
high contrast, and Tokyo Night, Rosé Pine, Kanagawa, Catppuccin, Gruvbox,
Nord, Dracula, Everforest and Solarized, each with its variants. By default
bastian follows your terminal's light or dark background.

```sh {norun}
bastian themes               # pick one with a live preview; Enter saves it
bastian -t nord README.md    # use a theme just this once
```

![The theme picker previewing themes on a sample document](demo/themes.gif)

You can also write your own, or tweak a built-in one; see [themes](docs/themes.md).

## Configuration

`bastian config` opens `~/.config/bastian/config.toml` in your editor, starting
from a commented template:

```toml
theme = "auto"
width = 100
list_layout = "compact"
line_numbers = true
editor = "nvim"
```

Every setting can also come from a `BASTIAN_*` environment variable or a flag,
and `bastian --print-config` shows where each one came from. See
[configuration](docs/configuration.md) for all of them.

## Documentation

- [The file list](docs/file-list.md): navigating, filtering and shaping the list
- [The reader](docs/reader.md): keys, search, table of contents, links,
  images, editing, remote documents and MDX
- [The command line](docs/cli.md): inputs, printing, options and commands
- [Running code blocks](docs/code-blocks.md): keys, attributes and interpreters
- [Themes](docs/themes.md): built-in themes, the picker and your own
- [Configuration](docs/configuration.md): every setting, environment variables
  and precedence

## License

[MIT](LICENSE)
