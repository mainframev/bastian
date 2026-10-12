# The command line

Besides the TUI, bastian reads single documents from anywhere and can print
them instead of opening the reader, for pipes, pagers and scripts.

← [README](../README.md)

## What it reads

```sh {norun}
bastian                           # browse the current directory
bastian docs/                     # browse another directory
bastian README.md                 # read a file
bastian src/main.rs               # source files are highlighted, with line numbers
gh pr view 12 --json body -q .body | bastian -    # read stdin
bastian https://example.com/doc.md                 # any Markdown URL
bastian github.com/owner/repo                      # a repository's README
bastian gitlab.com/group/project                   # GitLab too
bastian https://github.com/o/r/blob/main/docs/guide.md   # file pages are read raw
```

See [the reader](reader.md#remote-documents) for how links in remote
documents are followed.

## Printing

```sh {norun}
bastian -p README.md                       # print instead of opening the TUI
bastian -p -w 80 README.md                 # at most 80 columns wide
bastian --pager README.md                  # page it through $PAGER (default less -R)
bastian --color always README.md | less -R # keep the colors in a pipe
bastian -p README.md > plain.txt           # no colors when not a terminal
```

Printed output is colored when it goes to a terminal. `NO_COLOR` turns that
off, `CLICOLOR_FORCE=1` turns it on for pipes, and `--color` (or the `color`
setting) overrides both. Without `-w`, the width is the terminal's (up to 120
columns), or 80 when there's no terminal. Set `pager = true` to always page
instead of opening the TUI.

## Options

| Option                     | Effect                                                               |
| -------------------------- | -------------------------------------------------------------------- |
| `-w`, `--width <N>`        | maximum document width (default 100)                                 |
| `-t`, `--theme <NAME>`     | `auto`, `dark`, `light` or a theme name ([themes](themes.md))        |
| `-p`, `--print`            | print the rendered document                                          |
| `--pager`                  | show it in `$PAGER`                                                  |
| `--color <WHEN>`           | `auto`, `always` or `never`, for printed output                      |
| `-n`, `--line-numbers`     | show line numbers in the reader                                      |
| `-a`, `--all`              | also list hidden and gitignored files                                |
| `-y`, `--yes`              | run code blocks without asking ([code blocks](code-blocks.md))       |
| `--mouse`                  | scroll with the mouse wheel                                          |
| `--images`, `--no-images`  | draw pictures, or keep alt text                                      |
| `--no-logo`                | hide the logo above the file list                                    |
| `--selector <STYLE>`       | how the file list marks the selected file                            |
| `--list-layout <LAYOUT>`   | `cards` or `compact` file list                                       |
| `--sort <ORDER>`           | file list order: `modified`, `created` or `name`                     |
| `--config <PATH>`          | use another config file ([configuration](configuration.md))          |
| `--print-config`           | print the effective settings and where each came from                |
| `-h`, `--help`             | help                                                                 |
| `-V`, `--version`          | version                                                              |

## Commands

```sh {norun}
bastian themes                    # pick a theme with a live preview and save it
bastian config                    # open the config file in your editor
bastian completions zsh > ~/.zfunc/_bastian   # also bash, fish, elvish, powershell
bastian man > bastian.1 && man ./bastian.1    # the man page
```

`bastian themes` prints the theme list instead when its output isn't a
terminal. To read a file literally named `config` or `themes`, write
`./config`.
