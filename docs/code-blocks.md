# Running code blocks

Runbooks and setup guides can be run from the page: fenced code blocks in
`sh`, `bash`, `zsh`, `fish`, `python` (`py`, `python3`) or `node` (`js`,
`javascript`) run from the reader, with their output streamed underneath.

← [README](../README.md)

## Running

| Key                | Action                                                      |
| ------------------ | ----------------------------------------------------------- |
| `Tab`, `Shift-Tab` | focus the next/previous runnable block                      |
| `Enter`, `r`       | run the focused block                                       |
| `R`                | run every block from the focused one, stopping at a failure |
| `x`                | stop running blocks                                         |
| `o`                | show/hide the focused block's output                        |
| `y`                | copy the focused block                                      |

Each block sits in a frame with its name and language in the top border, and
`▶` there when bastian can run it. A `┊` on the left marks a long line that
wrapped. The focused block's frame turns the theme's accent color; `y` copies
the block's source, never the frame.

bastian asks before each run. `-y` (or `confirm = "never"`) skips the
question, except for blocks marked `confirm`. Try it on bastian's own README:
`bastian README.md`, then `Tab` and `Enter`.

## Attributes

Put attributes in braces after the language:

````markdown
```sh {name=deploy cwd=./infra env=STAGE=prod timeout=5m confirm}
./deploy.sh
```
````

| Attribute       | Effect                                                            |
| --------------- | ----------------------------------------------------------------- |
| `name=...`      | label shown on the block and in prompts; quote values with spaces |
| `cwd=...`       | working directory, relative to the document's directory           |
| `env=KEY=VALUE` | environment variable; repeat for more                             |
| `timeout=30s`   | stop the block after `ms`, `s`, `m` or `h`                        |
| `confirm`       | always ask before running, even with `--yes`                      |
| `norun`         | never run this block                                              |

## Other languages

Add or change interpreters in the [config file](configuration.md). The block
source is appended as the last argument:

```toml
[interpreters]
ruby = ["ruby", "-e"]
python = ["uv", "run", "python", "-c"]
fish = []             # an empty list stops a language from running
```

## How blocks run

- In the document's directory (or `cwd`), with no terminal attached, so
  interactive programs (editors, ssh, password prompts) don't work.
- Each block in its own process group: stopping it, or quitting bastian, stops
  everything it started.
- Output is kept up to the last 2000 lines.
- Blocks in remote documents run in the current directory and always ask
  first, even with `--yes`.
