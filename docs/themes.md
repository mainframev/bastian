# Themes

Themes color both the document and its code blocks. bastian comes with
popular ones and loads your own from TOML files.

← [README](../README.md)

## Built-in themes

| Family      | Themes                                                                                             |
| ----------- | -------------------------------------------------------------------------------------------------- |
| Terminal    | `terminal`: the terminal's own 16 colors, on a dark or light background                            |
| Neutral     | `neutral-dark`, `neutral-light`                                                                    |
| Accessible  | `high-contrast`                                                                                    |
| Tokyo Night | `tokyonight-night` (`tokyonight`), `tokyonight-storm`, `tokyonight-moon`, `tokyonight-day`         |
| Rosé Pine   | `rose-pine`, `rose-pine-moon`, `rose-pine-dawn`                                                    |
| Kanagawa    | `kanagawa-wave` (`kanagawa`), `kanagawa-dragon`, `kanagawa-lotus`                                  |
| Catppuccin  | `catppuccin-mocha` (`catppuccin`), `catppuccin-macchiato`, `catppuccin-frappe`, `catppuccin-latte` |
| Gruvbox     | `gruvbox-dark` (`gruvbox`), `gruvbox-light`                                                        |
| Nord        | `nord`                                                                                             |
| Dracula     | `dracula`                                                                                          |
| Everforest  | `everforest-dark` (`everforest`), `everforest-light`                                               |
| Solarized   | `solarized-dark` (`solarized`), `solarized-light`                                                  |

## Picking one

```sh {norun}
bastian themes               # pick one with a live preview; Enter saves it
bastian -t nord README.md    # use a theme just this once
```

`bastian themes` lists the themes in a pane on the right and previews each one
on a sample document as you move through the list (`j`/`k`, `↓`/`↑`, `g`/`G`).
The document re-wraps beside the pane, so the preview is how it will read.
Each theme shows whether it's dark or light, and the one in use is ticked.
Widen the pane and a swatch of its colors appears, on the theme's own surface,
with more colors the wider it gets. `Enter` saves the
highlighted one as `theme` in your config file, keeping your other settings and
comments; `Esc`, `q` or `T` leave things as they were.

Press `T` while reading or browsing for the same picker; the document or the
file list narrows beside it. `<` and `>` narrow or widen the pane, from 16
columns up to half the screen, and with `--mouse` so does clicking the `<` or
`>` in its header; the width lasts until bastian quits. With the table of
contents open, both stay while the document keeps 40 columns between them;
otherwise the table of contents is hidden until the picker closes. On screens
narrower than 40 columns the picker covers the screen.

![Switching themes with T while reading](../demo/reader-themes.gif)

Piped, `bastian themes` prints the list with each theme's colors.

![The theme picker previewing themes on a sample document](../demo/themes.gif)

## Light and dark

The default, `theme = "auto"`, asks the terminal for its background color and
uses `dark_theme` or `light_theme`. `"dark"` and `"light"` force one of the
two:

```toml
theme = "auto"                  # or "dark", "light", or a theme name
dark_theme = "rose-pine"
light_theme = "rose-pine-dawn"
```

`bastian themes` marks which theme each of these settings points at.

## Background

The reader and the file browser sit on the theme's own background, so its
colors look the same in every terminal. `terminal` keeps the terminal's
background. To keep the terminal's with every theme (for a transparent or
blurred window, say), set:

```toml
background = "terminal"  # or "theme", the default (--background)
```

Printed and piped output never paints the background. The terminal's padding
around the grid keeps its own color.

## Your own themes

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

Without `extends`, a theme sets `appearance` (`"dark"`, `"light"`, or `"any"`
for one that suits both) and every color:

- `[ui]` colors the document: `text`, `muted`, `subtle`, `surface`, `accent`,
  `h1_fg`, `h1_bg`, `heading`, `link`, `code`, `match_bg`, `current_bg`,
  `err`, `ok`, `warn`.
- `[ui]` may also set these; each one left out takes the color in brackets,
  so older themes keep working:
  - `heading_2`, `heading_3`, `heading_4` color headings of that level, and
    `heading_5` levels 5 and 6 (`heading`).
  - `heading_underline` colors the rule under level-2 headings (`muted`).
  - `bullet_1`, `bullet_2`, `bullet_3` color list bullets (`•`, `◦`, `▸`)
    by nesting depth, cycling for deeper lists (`accent`).
  - `background` colors the page behind the reader and the browser
    (`default`, the terminal's own). It must differ from `surface`, which
    code, the status bar and popups sit on.
- `[syntax]` colors code blocks: `comment`, `keyword`, `string`, `number`,
  `constant`, `function`, `type`, `operator`, `punctuation`, `tag`,
  `attribute`, `inserted`, `deleted`.

A color is `#rrggbb`, one of the 16 ANSI color names (`black`, `red`, `green`,
`yellow`, `blue`, `magenta`, `cyan`, `white`, and each with a `bright-`
prefix), which show in your terminal's palette, or `default` for the
terminal's own foreground or background.

The built-in themes in the repository's [`themes/`](../themes) directory are
complete examples. A file named after a built-in replaces it; to tweak a
built-in in place, extend it under its own name (`extends = "nord"` in
`themes/nord.toml`).
