## launchr

A fast terminal dashboard to launch, monitor, and manage apps, bookmarks, SSH hosts, scripts, notifications, and shell history. Built in Rust with ratatui.

### Build
- Prerequisites: Rust stable, Cargo
- Build: `cargo build --release`
- Run (dev): `cargo run`

Tested on Linux/macOS/Windows. On Windows, run in a true terminal (Windows Terminal/PowerShell).

### Keybindings
- Global: `q` quit, `Tab` next section, `j/k` or `↓/↑` navigate, `Enter` activate
- Lists: `n` new, `d` delete, `r` refresh, `t` toggle details, `PgUp`/`PgDn` `Home`/`End`
- Search: `/` open fuzzy search (scoped to current section), type to filter, `Enter` jump, `Esc` close
- Help: `?` overlay
- Apps: `Enter` launch, `s` stop process
- SSH: `x` disconnect latest session
- Scripts: `S` schedule selected script (example every 60s)

Sections: `1` Dashboard, `2` Apps, `3` Bookmarks, `4` SSH, `5` Scripts, `6` Notifications, `7` History

### Features
- Dashboard: running processes (scrollable), active SSH sessions, recent notifications
- Apps: auto-scan PATH for executables; show running processes via sysinfo
- Bookmarks: open files/dirs/URLs with platform-native open
- SSH: connect via system SSH in a new terminal when possible
- Scripts: run commands; simple scheduling (example)
- History: browse and run recent shell commands
- Context-aware fuzzy search `/` per section
- Dynamic header: username, time, arch, detected shell, CPU cores and average usage

### Shell detection & history
- Detects shell: PowerShell (Windows), Bash, Zsh, or Fish
- Loads recent history from:
  - PowerShell: `%APPDATA%/Microsoft/Windows/PowerShell/PSReadLine/ConsoleHost_history.txt`
  - Bash: `~/.bash_history`
  - Zsh: `~/.zsh_history`
  - Fish: `~/.local/share/fish/fish_history` (or `%APPDATA%/fish/fish_history` on Windows)
- Header shows the detected shell

### Configuration
Config is stored at:
- Linux: `~/.config/launchr/config.toml`
- macOS: `~/Library/Application Support/launchr/config.toml`
- Windows: `%APPDATA%/launchr/config.toml`

On first run, a default file is created. Example:

```toml
bookmarks = [
  { name = "Home", path = "~", bookmark_type = "directory" },
  { name = "Rust", path = "https://www.rust-lang.org", bookmark_type = "url" },
]

ssh_hosts = [
  { name = "Prod", host = "prod.example.com", port = 22, user = "ubuntu" },
]

scripts = [
  { name = "List", command = "ls -la", description = "List current dir" },
]
```

You can also add entries in-app:
- Bookmarks: `n` then `name|path|type` (type: `file`|`directory`|`url`)
- SSH: `n` then `name|user@host:port`
- Scripts: `n` then `name|command[|description]`

### Notes
- Uses `sysinfo` for processes, `which` for PATH resolution, and `crossterm`/`ratatui` for TUI.
- Some actions spawn external programs; ensure they exist in PATH (e.g., `ssh`).


### License
MIT. See the `LICENSE` file in the repository.

