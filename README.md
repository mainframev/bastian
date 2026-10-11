# bastian

Read Markdown in the terminal, and run its code blocks.

bastian is a glow-style terminal reader with one addition: fenced code blocks
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
bastian README.md       # read a file
cat notes.md | bastian -
bastian -p README.md    # print instead of opening the TUI
bastian -y runbook.md   # run blocks without confirming each one
bastian -n README.md    # show line numbers
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
| `L` | list links; follow one with `Enter` |
| `e` | edit the file in `$VISUAL`, `$EDITOR` or `vi` |
| `#` | toggle line numbers |
| `?` | key help |
| `Esc`, `h` | back to the previous document or the browser |
| `q` | quit |

In the browser: `j/k` to move, `h/l` (or `←/→`) for the previous/next page,
`ctrl-d/u` and `ctrl-f/b` to page, `/` to filter,
`Enter` to open, `e` to edit, `r` to rescan for new or deleted files, `?` for
all keys, `q` to quit.

The open document reloads when the file changes on disk.

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
theme = "dark"        # or "light"
width = 100           # maximum document width
confirm = "always"    # "never" behaves like --yes
line_numbers = false  # true behaves like --line-numbers

[interpreters]
# The block source is appended as the last argument.
ruby = ["ruby", "-e"]
python = ["uv", "run", "python", "-c"]
fish = []             # an empty list stops a language from running
```

## Development

```sh {name=check}
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
