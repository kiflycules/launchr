use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct NetworkConnection {
    pub protocol: String,
    pub local_addr: String,
    pub remote_addr: String,
    pub state: String,
    #[allow(dead_code)]
    pub pid: Option<u32>,
    pub process_name: String,
}

#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub ip_addresses: Vec<String>,
    pub mac_address: String,
    pub status: String, // up/down
    #[allow(dead_code)]
    pub rx_bytes: u64,
    #[allow(dead_code)]
    pub tx_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct PortUsage {
    pub port: u16,
    pub protocol: String,
    pub process_name: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkView {
    Connections,
    Interfaces,
    Ports,
}

pub struct NetworkModule {
    pub connections: Vec<NetworkConnection>,
    pub interfaces: Vec<NetworkInterface>,
    pub listening_ports: Vec<PortUsage>,
    pub filter_state: Option<String>, // ESTABLISHED, LISTEN, etc.
    pub current_view: NetworkView,
}

impl NetworkModule {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            interfaces: Vec::new(),
            listening_ports: Vec::new(),
            filter_state: None,
            current_view: NetworkView::Connections,
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        match self.current_view {
            NetworkView::Connections => self.refresh_connections()?,
            NetworkView::Interfaces => self.refresh_interfaces()?,
            NetworkView::Ports => self.refresh_listening_ports()?,
        }
        Ok(())
    }

    pub fn refresh_connections(&mut self) -> Result<()> {
        self.connections.clear();

        #[cfg(target_os = "linux")]
        {
            // Use ss command for better performance than netstat
            let output = Command::new("ss").args(&["-tupan"]).output();

            if let Ok(output) = output {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines().skip(1) {
                        if let Some(conn) = self.parse_ss_line(line) {
                            if self.filter_state.is_none()
                                || self.filter_state.as_ref() == Some(&conn.state)
                            {
                                self.connections.push(conn);
                            }
                        }
                    }
                    return Ok(());
                }
            }
            // Fallback to netstat
            self.use_netstat()?;
        }

        #[cfg(target_os = "macos")]
        {
            self.use_netstat()?;
        }

        #[cfg(target_os = "windows")]
        {
            let output = Command::new("netstat").args(&["-ano"]).output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().skip(4) {
                    if let Some(conn) = self.parse_netstat_windows(line) {
                        if self.filter_state.is_none()
                            || self.filter_state.as_ref() == Some(&conn.state)
                        {
                            self.connections.push(conn);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn use_netstat(&mut self) -> Result<()> {
        let output = Command::new("netstat").args(&["-an"]).output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(conn) = self.parse_netstat_line(line) {
                    if self.filter_state.is_none()
                        || self.filter_state.as_ref() == Some(&conn.state)
                    {
                        self.connections.push(conn);
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn parse_ss_line(&self, line: &str) -> Option<NetworkConnection> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            return None;
        }

        let protocol = parts[0].to_uppercase();
        let state = parts[1].to_string();
        let local_addr = parts[4].to_string();
        let remote_addr = if parts.len() > 5 {
            parts[5].to_string()
        } else {
            "*:*".to_string()
        };

        // Try to extract PID from users column
        let (pid, process_name) = if parts.len() > 6 {
            self.extract_pid_from_users(parts[6])
        } else {
            (None, String::from("?"))
        };

        Some(NetworkConnection {
            protocol,
            local_addr,
            remote_addr,
            state,
            pid,
            process_name,
        })
    }

    #[cfg(target_os = "linux")]
    fn extract_pid_from_users(&self, users_field: &str) -> (Option<u32>, String) {
        // Format is like: users:(("process",pid=1234,fd=3))
        if let Some(pid_start) = users_field.find("pid=") {
            let pid_str = &users_field[pid_start + 4..];
            if let Some(pid_end) = pid_str.find(',') {
                let pid = pid_str[..pid_end].parse::<u32>().ok();

                // Extract process name
                if let Some(name_start) = users_field.find("((\"") {
                    if let Some(name_end) = users_field[name_start + 3..].find('\"') {
                        let name =
                            users_field[name_start + 3..name_start + 3 + name_end].to_string();
                        return (pid, name);
                    }
                }
                return (pid, String::from("?"));
            }
        }
        (None, String::from("?"))
    }

    #[allow(dead_code)]
    fn parse_netstat_line(&self, line: &str) -> Option<NetworkConnection> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return None;
        }

        let protocol = parts[0].to_uppercase();
        if !protocol.starts_with("TCP") && !protocol.starts_with("UDP") {
            return None;
        }

        let local_addr = parts.get(3)?.to_string();
        let remote_addr = parts.get(4).unwrap_or(&"*:*").to_string();
        let state = parts.get(5).unwrap_or(&"").to_string();

        Some(NetworkConnection {
            protocol,
            local_addr,
            remote_addr,
            state,
            pid: None,
            process_name: String::from("?"),
        })
    }

    #[cfg(target_os = "windows")]
    fn parse_netstat_windows(&self, line: &str) -> Option<NetworkConnection> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return None;
        }

        let protocol = parts[0].to_uppercase();
        if protocol != "TCP" && protocol != "UDP" {
            return None;
        }

        let local_addr = parts[1].to_string();
        let remote_addr = parts[2].to_string();
        let state = if protocol == "TCP" && parts.len() > 3 {
            parts[3].to_string()
        } else {
            String::from("-")
        };

        let pid = if protocol == "TCP" && parts.len() > 4 {
            parts[4].parse::<u32>().ok()
        } else if protocol == "UDP" && parts.len() > 3 {
            parts[3].parse::<u32>().ok()
        } else {
            None
        };

        let process_name = String::from("?");

        Some(NetworkConnection {
            protocol,
            local_addr,
            remote_addr,
            state,
            pid,
            process_name,
        })
    }

    pub fn refresh_interfaces(&mut self) -> Result<()> {
        self.interfaces.clear();

        #[cfg(target_os = "linux")]
        {
            // Use ip command
            let output = Command::new("ip")
                .args(&["-brief", "addr", "show"])
                .output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some(iface) = self.parse_ip_addr_line(line) {
                        self.interfaces.push(iface);
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // Use ifconfig
            let output = Command::new("ifconfig").output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                self.parse_ifconfig(&stdout);
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Use ipconfig
            let output = Command::new("ipconfig").args(&["/all"]).output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                self.parse_ipconfig(&stdout);
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn parse_ip_addr_line(&self, line: &str) -> Option<NetworkInterface> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let name = parts[0].to_string();
        let status = parts[1].to_lowercase();
        let ip_addresses: Vec<String> = parts
            .iter()
            .skip(2)
            .filter(|s| s.contains('.') || s.contains(':'))
            .map(|s| s.split('/').next().unwrap_or(s).to_string())
            .collect();

        Some(NetworkInterface {
            name,
            ip_addresses,
            mac_address: String::from("N/A"),
            status,
            rx_bytes: 0,
            tx_bytes: 0,
        })
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[allow(dead_code)]
    fn parse_ifconfig(&mut self, output: &str) {
        let mut current_interface: Option<NetworkInterface> = None;

        for line in output.lines() {
            let trimmed = line.trim();

            // New interface starts
            if !line.starts_with('\t') && !line.starts_with(' ') && line.contains(':') {
                if let Some(iface) = current_interface.take() {
                    self.interfaces.push(iface);
                }

                let name = line.split(':').next().unwrap_or("").to_string();
                current_interface = Some(NetworkInterface {
                    name,
                    ip_addresses: Vec::new(),
                    mac_address: String::from("N/A"),
                    status: String::from("up"),
                    rx_bytes: 0,
                    tx_bytes: 0,
                });
            } else if let Some(ref mut iface) = current_interface {
                if trimmed.starts_with("inet ") {
                    if let Some(addr) = trimmed.split_whitespace().nth(1) {
                        iface.ip_addresses.push(addr.to_string());
                    }
                } else if trimmed.starts_with("ether ") {
                    if let Some(mac) = trimmed.split_whitespace().nth(1) {
                        iface.mac_address = mac.to_string();
                    }
                }
            }
        }

        if let Some(iface) = current_interface {
            self.interfaces.push(iface);
        }
    }

    #[cfg(target_os = "windows")]
    fn parse_ipconfig(&mut self, output: &str) {
        let mut current_interface: Option<NetworkInterface> = None;

        for line in output.lines() {
            let trimmed = line.trim();

            // New adapter starts
            if line.starts_with("Ethernet adapter") || line.starts_with("Wireless LAN adapter") {
                if let Some(iface) = current_interface.take() {
                    self.interfaces.push(iface);
                }

                let name = line
                    .split(':')
                    .next()
                    .map(|s| {
                        s.replace("Ethernet adapter ", "")
                            .replace("Wireless LAN adapter ", "")
                    })
                    .unwrap_or_else(|| "Unknown".to_string());

                current_interface = Some(NetworkInterface {
                    name,
                    ip_addresses: Vec::new(),
                    mac_address: String::from("N/A"),
                    status: String::from("up"),
                    rx_bytes: 0,
                    tx_bytes: 0,
                });
            } else if let Some(ref mut iface) = current_interface {
                if trimmed.starts_with("IPv4 Address") || trimmed.starts_with("IPv6 Address") {
                    if let Some(addr) = trimmed.split(':').nth(1) {
                        let clean_addr = addr.trim().trim_end_matches("(Preferred)").trim();
                        iface.ip_addresses.push(clean_addr.to_string());
                    }
                } else if trimmed.starts_with("Physical Address") {
                    if let Some(mac) = trimmed.split(':').nth(1) {
                        iface.mac_address = mac.trim().to_string();
                    }
                }
            }
        }

        if let Some(iface) = current_interface {
            self.interfaces.push(iface);
        }
    }

    pub fn refresh_listening_ports(&mut self) -> Result<()> {
        self.listening_ports.clear();

        #[cfg(target_os = "linux")]
        {
            let output = Command::new("ss").args(&["-tulpn"]).output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().skip(1) {
                    if let Some(port) = self.parse_ss_listening(line) {
                        self.listening_ports.push(port);
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let output = Command::new("netstat").args(&["-ano"]).output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().skip(4) {
                    if line.contains("LISTENING") {
                        if let Some(port) = self.parse_netstat_listening_windows(line) {
                            self.listening_ports.push(port);
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let output = Command::new("netstat").args(&["-anv"]).output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("LISTEN") {
                        if let Some(port) = self.parse_netstat_listening_macos(line) {
                            self.listening_ports.push(port);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn parse_ss_listening(&self, line: &str) -> Option<PortUsage> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            return None;
        }

        let protocol = parts[0].to_uppercase();
        let local_addr = parts[4];

        // Extract port from address (format: *:port or ip:port)
        let port = local_addr.split(':').last()?.parse::<u16>().ok()?;

        let (pid, process_name) = if parts.len() > 6 {
            self.extract_pid_from_users(parts[6])
        } else {
            (None, String::from("?"))
        };

        Some(PortUsage {
            port,
            protocol,
            process_name,
            pid: pid.unwrap_or(0),
        })
    }

    #[cfg(target_os = "windows")]
    fn parse_netstat_listening_windows(&self, line: &str) -> Option<PortUsage> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            return None;
        }

        let protocol = parts[0].to_uppercase();
        let local_addr = parts[1];
        let port = local_addr.split(':').last()?.parse::<u16>().ok()?;
        let pid = parts[4].parse::<u32>().ok()?;

        Some(PortUsage {
            port,
            protocol,
            process_name: String::from("?"),
            pid,
        })
    }

    #[cfg(target_os = "macos")]
    fn parse_netstat_listening_macos(&self, line: &str) -> Option<PortUsage> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return None;
        }

        let protocol = parts[0].to_uppercase();
        let local_addr = parts[3];
        let port = local_addr.split('.').last()?.parse::<u16>().ok()?;

        Some(PortUsage {
            port,
            protocol,
            process_name: String::from("?"),
            pid: 0,
        })
    }

    pub fn toggle_filter(&mut self, state: &str) {
        if self.filter_state.as_ref() == Some(&state.to_string()) {
            self.filter_state = None;
        } else {
            self.filter_state = Some(state.to_string());
        }
    }

    #[allow(dead_code)]
    pub fn clear_filter(&mut self) {
        self.filter_state = None;
    }
}
