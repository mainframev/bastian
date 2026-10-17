# bastian

![bastian: a terminal Markdown reader](https://upnsbzkykttnbdp1.public.blob.vercel-storage.com/assets/bastian.png)

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

![Browsing, filtering, searching and the table of contents in bastian](https://upnsbzkykttnbdp1.public.blob.vercel-storage.com/demo/usage.gif)

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

## Features

`?` shows every key in the app, `Esc` goes back one step and `q` quits. The
linked docs pages show most features in short GIFs.

### Browsing a folder

Run `bastian` in a folder (or `bastian docs/`) to see its Markdown files,
newest first. More in [the file list](docs/file-list.md).

- `j/k` to move, `/` to [filter](docs/file-list.md#filtering) by path or
  title, `Enter` to open, `e` to edit.
- Hidden and gitignored files are skipped unless you pass `-a`.
- [Shape the list](docs/file-list.md#shaping-the-list): cards or a compact
  one-row layout, several selection styles, sorted by modified or created
  date or by name.

### Reading

Everything below is in [the reader](docs/reader.md).

- [Table of contents](docs/reader.md#table-of-contents): `t` shows it beside
  the document. `1`–`9` and `0` jump to its sections, pressing the same number
  again walks into that section, and `J/K` step through headings.
- [Search](docs/reader.md#search) with `/` and `n/N`. `:` or `ctrl-g`
  [goes to a line](docs/reader.md#line-numbers), and `#` shows line numbers.
- [Links](docs/reader.md#links): `L` lists them, filterable, and `s/S` step
  through them in place. Markdown files open in bastian, `[[wiki links]]`
  included, and links are clickable in terminals that support it.
  [History](docs/reader.md#history): `H` lists the documents read this session.
- [Editing and live reload](docs/reader.md#editing-and-live-reload): `ctrl-e`
  opens your editor at the line you're reading, and the document reloads when
  the file changes.
- [Reading position](docs/reader.md#reading-position): reopening a file
  scrolls back to the section you left.
- [Copying](docs/reader.md#copying): `y` copies a code block, `c` the whole
  document, and `p` shows and copies the file path. It uses OSC 52, so it
  works over ssh.
- Mouse (`mouse` setting): the wheel scrolls, and the link under the pointer
  is shaded ([keys](docs/reader.md#keys)).
- [Themes](docs/themes.md#picking-one): `T` switches the theme without
  leaving the document.

### Markdown it lays out

- Tables fitted to the width, highlighted code, task lists, footnotes, GitHub
  alerts (`> [!NOTE]`, …) and CJK text wrapped between characters.
- [Front matter](docs/reader.md#mdx-and-front-matter) as a table of keys and
  values.
- [Inline HTML and `==highlights==`](docs/reader.md#inline-html-and-highlights):
  `<kbd>`, `<mark>`, `<b>`, `<s>` and friends are styled, and HTML comments
  are hidden.
- [Math](docs/reader.md#math): TeX in `$…$`, `$$…$$` and ` ```math ` reads as
  Unicode text.
- [`<details>`](docs/reader.md#details) blocks fold: `d` focuses one and
  `Enter` opens or closes it.
- [Images](docs/reader.md#images) are drawn as pictures where the terminal
  can, and `i` toggles them.
- [MDX](docs/reader.md#mdx-and-front-matter) files and source files are
  supported too.
- [Mermaid diagrams](docs/reader.md#diagrams) are drawn with box-drawing
  characters when built with `--features mermaid`, and `v` shows their source.

## The CLI

bastian also reads documents from anywhere, and prints them for pipes and
pagers:

```sh {norun}
bastian README.md                    # read a file in the TUI
bastian -                            # read stdin
cat notes.md | bastian               # piped input is read without -
bastian https://example.com/doc.md   # a Markdown URL
bastian github.com/owner/repo        # a repository's README (GitLab too)
bastian -p README.md                 # print it instead
bastian -p -w 80 README.md           # at most 80 columns wide
bastian --pager README.md            # page it through $PAGER
bastian --color always README.md | less -R
```

Every input form, option and command is in [the command line](docs/cli.md),
and [remote documents](docs/reader.md#remote-documents) covers URLs and
repositories.

## Running code blocks

Shell, Python and Node blocks can run right in the reader, with their output
underneath. Try it here: open this file with `bastian README.md`, press `Tab`
to focus the block below and `Enter` to run it.

```sh {name=hello}
echo "hello from $(basename "$PWD")"
uname -sm
```

`R` runs every block from the focused one, stopping at a failure, and `x`
stops them. Blocks ask before running unless you pass `--yes`.

Attributes like `{cwd=./infra timeout=5m confirm}` control how a block runs;
see [code blocks](docs/code-blocks.md).

## Themes

Built in: `terminal` (your terminal's own colors), neutral dark and light,
high contrast, and Tokyo Night, Rosé Pine, Kanagawa, Catppuccin, Gruvbox,
Nord, Dracula, Everforest and Solarized, each with its variants. By default
bastian follows your terminal's light or dark background and paints the
theme's [page background](docs/themes.md#background).

```sh {norun}
bastian themes               # pick one with a live preview; Enter saves it
bastian -t nord README.md    # use a theme just this once
```

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
