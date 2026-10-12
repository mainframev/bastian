# Browsing and reading the sample docs in demo/docs. Regenerate with
# `just demo` (or `vhs demo/usage.tape` from the repository root, with
# bastian on PATH).
Output demo/usage.gif

Set Shell "bash"
Set FontFamily "JetBrainsMono Nerd Font"
Set FontSize 18
Set Width 1440
Set Height 900
Set Padding 24
Set Margin 20
Set MarginFill "#191724"
Set WindowBar Colorful
Set BorderRadius 10
Set Theme "rose-pine"
Set Framerate 30
Set TypingSpeed 90ms

# A throwaway home with a copy of the docs, so the browser shows
# `~/atlas` and nothing reads or writes your config.
Hide
Type `D=$(mktemp -d /tmp/bastian.XXXX) && mkdir "$D/atlas" && cp -R demo/docs/. "$D/atlas/"`
Enter
Type `export HOME=$(cd "$D" && pwd -P) && cd "$HOME/atlas" && clear`
Enter
Show

Type "bastian"
Sleep 500ms
Enter
Sleep 2s

# Move through the list, then filter it.
Down@500ms 3
Sleep 600ms
Type "/"
Sleep 300ms
Type "api"
Sleep 800ms
Enter
Sleep 500ms
Enter
Sleep 2s

# Read: scroll through the table and code.
Down@120ms 18
Sleep 1.5s

# Search, then jump between matches.
Type "/"
Sleep 300ms
Type "token"
Sleep 600ms
Enter
Sleep 1s
Type "n"
Sleep 1s
Type "n"
Sleep 1.2s
Escape
Sleep 500ms

# The outline: jump to a heading.
Type "O"
Sleep 1.2s
Down@400ms 4
Sleep 600ms
Enter
Sleep 1.8s

# Back to the list and open another document.
Escape
Sleep 1s
Escape
Sleep 600ms
Up@400ms 3
Sleep 400ms
Enter
Sleep 2s
Down@120ms 12
Sleep 2s
