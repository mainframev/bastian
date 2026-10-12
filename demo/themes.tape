# The theme picker. Regenerate with `just demo` (or `vhs demo/themes.tape`
# from the repository root, with bastian on PATH).
Output demo/themes.gif

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 720
Set Padding 16
Set Theme "rose-pine"
Set TypingSpeed 60ms
Set Framerate 20

# A config of its own, so saving a theme doesn't touch yours.
Hide
Type "export XDG_CONFIG_HOME=$(mktemp -d /tmp/bastian.XXXX) && clear"
Enter
Show

Type "bastian themes"
Sleep 400ms
Enter
Sleep 1.5s

Down@600ms 12
Sleep 1s
Enter
Sleep 2.5s
