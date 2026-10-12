# Configuration

Every setting is optional. They live in `~/.config/bastian/config.toml` (or
`$XDG_CONFIG_HOME/bastian/config.toml`), and each one can also come from the
environment or a flag.

← [README](../README.md)

```sh {norun}
bastian config            # open the file in your editor (created from a commented template)
bastian --print-config    # the effective settings, and where each one came from
bastian --config ~/work/bastian.toml README.md   # another file (also BASTIAN_CONFIG)
```

## All settings

```toml
# Look
theme = "auto"        # "auto" (match the terminal), "dark", "light" or a theme name
dark_theme = "rose-pine"        # used by "auto" on dark backgrounds, and "dark"
light_theme = "rose-pine-dawn"  # used by "auto" on light backgrounds, and "light"
width = 100           # maximum document width
line_numbers = false  # show line numbers in the reader (-n)
images = true         # draw pictures; `i` toggles them (--images, --no-images)
image_height = 24     # rows a picture takes up at most
tab_width = 4         # columns per tab in code
hyperlinks = true     # clickable links (OSC 8)
scrollbar = true      # show scrollbars
mouse = false         # scroll with the mouse wheel (--mouse)
remember_position = true  # reopen files where you left off

# File list
all = false           # also list hidden and gitignored files (-a)
sort = "modified"     # "modified", "created" or "name" (--sort)
selector = "bar"      # "bar", "thick", "block", "arrow" or "highlight" (--selector)
list_layout = "cards" # "cards" or "compact" (--list-layout)
dates = true          # show file dates
date_format = "%d %b %Y %H:%M %Z"  # strftime format of those dates
extensions = ["md", "markdown", "mdx", "mdown", "mkdn", "mkd"]  # files listed
ignore = []           # gitignore-style globs to leave out, e.g. ["drafts/"]
logo = true           # the logo at the top (--no-logo)

# Printing and running
color = "auto"        # printed output: "auto", "always" or "never" (--color)
pager = false         # show documents in $PAGER instead of the TUI (--pager)
confirm = "always"    # "never" runs code blocks without asking, like --yes
# editor = "nvim"     # for `e` and `bastian config`, before $VISUAL and $EDITOR

[interpreters]
# Command lines for code block languages; the source is appended last.
ruby = ["ruby", "-e"]
fish = []             # an empty list stops a language from running
```

The pages on [the file list](file-list.md), [the reader](reader.md),
[code blocks](code-blocks.md) and [themes](themes.md) explain these in
context.

## Environment variables

Each setting can also be set as `BASTIAN_<KEY>`, for example
`BASTIAN_THEME=nord`, `BASTIAN_LINE_NUMBERS=true` or
`BASTIAN_LIST_LAYOUT=compact`. Booleans accept `true`/`false`, `1`/`0`,
`yes`/`no` and `on`/`off`; lists are comma-separated
(`BASTIAN_EXTENSIONS=md,txt`). An empty variable counts as unset.
`[interpreters]` is only read from the file.

## Precedence

Defaults < config file < environment < command-line flags.
`bastian --print-config` prints each effective value with its source
(`default`, `config file`, `environment` or `flag`), and an invalid value
stops bastian with an error naming where it came from.
