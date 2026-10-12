# Switching themes while reading. Regenerate with `just demo` (or
# `vhs demo/reader-themes.tape` from the repository root).
Output demo/reader-themes.gif

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 720
Set Padding 16
Set Theme "rose-pine"
Set TypingSpeed 60ms
Set Framerate 20

Hide
Type "export XDG_CONFIG_HOME=$(mktemp -d /tmp/bastian.XXXX) && clear"
Enter
Show

Type "bastian --no-images README.md"
Sleep 400ms
Enter
Sleep 1.5s
Type "T"
Sleep 1.2s
Down@800ms 8
Sleep 600ms
Enter
Sleep 2s
