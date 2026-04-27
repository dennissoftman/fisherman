Output gifs/traceroute.gif

Set FontSize 16
Set Width 1200
Set Height 600
Set WindowBar Colorful
Set Theme "TokyoNight"
Set TypingSpeed 50ms
Env PATH "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

Type "fisherman -P" Enter
Sleep 1.5s

# Switch to Traceroute tab (4 when Speedtest is visible, 3 otherwise)
Type "4"
Sleep 0.5s
Type "i" Type "8.8.8.8" Enter

Sleep 15s

# Copy results to clipboard
Type "y"
Sleep 1s

Type "q"
