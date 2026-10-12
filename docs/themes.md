# Themes

Themes color both the document and its code blocks. bastian comes with
popular ones and loads your own from TOML files.

← [README](../README.md)

## Built-in themes

| Family      | Themes                                                                                             |
| ----------- | -------------------------------------------------------------------------------------------------- |
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

`bastian themes` previews each theme on a sample document as you move through
the list. `Enter` saves the highlighted one as `theme` in your config file,
keeping your other settings and comments; `Esc` leaves things as they were.
Press `T` while reading or browsing for the same picker. Piped,
`bastian themes` prints the list with each theme's colors.

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

Without `extends`, a theme sets `appearance` (`"dark"` or `"light"`) and every
color:

- `[ui]` colors the document: `text`, `muted`, `subtle`, `surface`, `accent`,
  `h1_fg`, `h1_bg`, `heading`, `link`, `code`, `match_bg`, `current_bg`,
  `err`, `ok`, `warn`.
- `[syntax]` colors code blocks: `comment`, `keyword`, `string`, `number`,
  `constant`, `function`, `type`, `operator`, `punctuation`, `tag`,
  `attribute`, `inserted`, `deleted`.

The built-in themes in the repository's [`themes/`](../themes) directory are
complete examples. A file named after a built-in replaces it; to tweak a
built-in in place, extend it under its own name (`extends = "nord"` in
`themes/nord.toml`).
