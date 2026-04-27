Output gifs/ping.gif

Set FontSize 16
Set Width 1200
Set Height 600
Set WindowBar Colorful
Set Theme "TokyoNight"
Set TypingSpeed 50ms
Env PATH "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

Type "fisherman -P" Enter
Sleep 3s

# Switch to Ping tab (3 when Speedtest is visible, 2 otherwise)
Type "3"
Sleep 0.5s
Type "i" Type "1.1.1.1" Enter
Sleep 3s

# Decrease interval (faster pings)
Type "---"
Sleep 3s

# Increase interval back
Type "+++"
Sleep 1s

Type "q"
