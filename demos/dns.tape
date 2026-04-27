Output gifs/dns.gif

Set FontSize 16
Set Width 1200
Set Height 600
Set WindowBar Colorful
Set Theme "TokyoNight"
Set TypingSpeed 50ms
Env PATH "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

Type "fisherman -P" Enter
Sleep 3s

# Switch to DNS tab (5 when Speedtest is visible, 4 otherwise)
Type "5"
Sleep 0.5s

# Lookup 1
Type "i" Type "google.com" Enter
Sleep 2s

# Cycle filter: IPv4 only → IPv6 only → both
Type "f"
Sleep 0.8s
Type "f"
Sleep 0.8s
Type "f"
Sleep 0.5s

# Lookup 2 — clear field with enough Backspaces then type new host
Type "i"
Backspace 20
Type "github.com" Enter
Sleep 2s

# Lookup 3
Type "i"
Backspace 20
Type "cloudflare.com" Enter
Sleep 2s

Type "q"
