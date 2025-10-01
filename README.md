## launchr

A fast terminal dashboard to launch, monitor, and manage apps, bookmarks, clipboard history, Docker containers, Git repositories, network connections, SSH hosts, scripts, notifications, and shell history. Built in Rust with ratatui.

### Build
- Prerequisites: Rust stable, Cargo
- Build: `cargo build --release`
- Run (dev): `cargo run`

Tested on Linux/macOS/Windows. On Windows, run in a true terminal (Windows Terminal/PowerShell).

### Keybindings
- Global: `q` quit, `Tab`/`Shift+Tab` next/previous section, `j/k` or `↓/↑` navigate, `Enter` activate
- Lists: `n` new, `d` delete, `r` refresh, `t` toggle details, `PgUp`/`PgDn` `Home`/`End`
- Search: `/` open fuzzy search (scoped to current section), type to filter, `Enter` jump, `Esc` close
- Help: `?` overlay
- Apps: `Enter` launch, `s` stop process (also from Dashboard)
- Clipboard: `Enter` copy to clipboard, `p` pin/unpin entry
- Docker: `Enter` exec into container, `v` switch view (containers/images), `a` toggle show all
- Network: `v` switch view (connections/interfaces/ports), `f` filter connections by state
- SSH: `x` disconnect latest session (terminates the ssh process)
- Scripts: `S` schedule selected script (example every 60s)
- Git: `Enter` open in editor, `S` scan for repositories

Sections: `1` Dashboard, `2` Apps, `3` Bookmarks, `4` Clipboard, `5` Docker, `6` Network, `7` SSH, `8` Scripts, `9` Git, `0` History/Notifications (toggle)

### Features
- Dashboard: running processes (scrollable), active SSH sessions, recent notifications
- Apps: auto-scan PATH for executables (on Windows, filters to .exe/.bat/.cmd/.ps1); show running processes via sysinfo
- Bookmarks: open files/dirs/URLs with platform-native open
- Clipboard: track clipboard history with timestamps, pin important entries, copy back to clipboard
- Docker: view and manage containers and images; exec into running containers; toggle between running and all containers
- Git: scan directories for repositories; view status, branch, uncommitted changes, ahead/behind commits; open repositories in editor
- Network: monitor active connections with state filtering; view network interfaces with IP/MAC addresses; list listening ports with process info
- SSH: connect via system SSH (uses `-p <port>`); auto-detects active sessions by scanning running `ssh` processes; disconnect (`x`) ends the underlying process
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

git_search_paths = [
  "~/Projects",
  "~/Documents/GitHub",
  "~/code",
]
```

You can also add entries in-app:
- Bookmarks: `n` then `name|path|type`
- SSH: `n` then `name|user@host:port`
- Scripts: `n` then `name|command|description`

### Module Details

**Clipboard Module**
- Stores clipboard content with metadata (type, timestamp)
- Pin frequently used entries to keep them persistent
- Supports text, command, and URL types
- Windows: uses `clip` command for clipboard operations

**Docker Module**
- Lists containers with status, image, and port information
- Lists Docker images with repository, tag, and size
- Exec into containers (opens new terminal window)
- Toggle between running containers and all containers
- Requires Docker CLI installed

**Git Module**
- Scans configurable search paths for Git repositories (default: ~/Projects, ~/Documents/GitHub, etc.)
- Displays repository status: clean, modified, ahead, behind
- Shows current branch, uncommitted changes count, and last commit message
- Opens repository in available editor (nvim, nano, code etc.)
- Requires Git CLI installed

**Network Module**
- Connections view: active network connections with protocol, addresses, state, and process info
- Interfaces view: network adapters with IP addresses, MAC addresses, and status
- Ports view: listening ports with process names and PIDs
- Filter connections by state (ESTABLISHED, LISTEN, etc.)
- Uses platform-specific commands: `ss`/`netstat` (Linux), `netstat` (macOS), `netstat`/`ipconfig` (Windows)

### Notes
- Uses `sysinfo` for processes, `which` for PATH resolution, and `crossterm`/`ratatui` for TUI.
- Some actions spawn external programs; ensure they exist in PATH (e.g., `ssh`, `docker`, `git`).
- Network monitoring requires appropriate permissions to view process information on some systems.


### License
MIT. See the `LICENSE` file in the repository.

