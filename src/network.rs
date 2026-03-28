use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use procfs::net::{TcpNetEntry, TcpState, UdpNetEntry};

/// Snapshot of a single network interface's counters.
#[derive(Clone, Debug)]
pub struct IfaceStats {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

/// Per-interface bandwidth rates (bytes/sec) computed from two snapshots.
#[derive(Clone, Debug, Default)]
pub struct IfaceRate {
    pub rx_bps: f64,
    pub tx_bps: f64,
}

/// A TCP connection with resolved details.
#[derive(Clone, Debug)]
pub struct TcpConn {
    pub local_addr: String,
    pub remote_addr: String,
    pub state: String,
    pub uid: u32,
    pub inode: u64,
}

/// A UDP socket with resolved details.
#[derive(Clone, Debug)]
pub struct UdpSocket {
    pub local_addr: String,
    pub remote_addr: String,
    pub uid: u32,
    pub inode: u64,
}

/// Holds all collected network data for a single poll cycle.
#[derive(Clone, Debug)]
pub struct NetworkData {
    pub interfaces: Vec<IfaceStats>,
    pub rates: HashMap<String, IfaceRate>,
    pub tcp_conns: Vec<TcpConn>,
    pub udp_sockets: Vec<UdpSocket>,
    pub tcp_state_counts: HashMap<String, usize>,
    pub total_rx_bps: f64,
    pub total_tx_bps: f64,
    pub bandwidth_history: HashMap<String, VecDeque<(f64, f64)>>,
}

pub struct NetworkCollector {
    prev_stats: Option<Vec<IfaceStats>>,
    prev_time: Option<Instant>,
    bandwidth_history: HashMap<String, VecDeque<(f64, f64)>>,
}

const MAX_HISTORY: usize = 60;

impl NetworkCollector {
    pub fn new() -> Self {
        Self {
            prev_stats: None,
            prev_time: None,
            bandwidth_history: HashMap::new(),

        }
    }

    pub fn collect(&mut self) -> NetworkData {
        let now = Instant::now();
        let interfaces = Self::read_interfaces();
        let rates = self.compute_rates(&interfaces, now);

        let mut total_rx_bps = 0.0;
        let mut total_tx_bps = 0.0;
        for rate in rates.values() {
            total_rx_bps += rate.rx_bps;
            total_tx_bps += rate.tx_bps;
        }

        // Update bandwidth history.
        for iface in &interfaces {
            let rate = rates.get(&iface.name).cloned().unwrap_or_default();
            let history = self
                .bandwidth_history
                .entry(iface.name.clone())
                .or_default();
            history.push_back((rate.rx_bps, rate.tx_bps));
            if history.len() > MAX_HISTORY {
                history.pop_front();
            }
        }

        self.prev_stats = Some(interfaces.clone());
        self.prev_time = Some(now);

        let tcp_conns = Self::read_tcp();
        let udp_sockets = Self::read_udp();

        let mut tcp_state_counts: HashMap<String, usize> = HashMap::new();
        for conn in &tcp_conns {
            *tcp_state_counts.entry(conn.state.clone()).or_insert(0) += 1;
        }

        NetworkData {
            interfaces,
            rates,
            tcp_conns,
            udp_sockets,
            tcp_state_counts,
            total_rx_bps,
            total_tx_bps,
            bandwidth_history: self.bandwidth_history.clone(),
        }
    }

    fn read_interfaces() -> Vec<IfaceStats> {
        let dev_status = match procfs::net::dev_status() {
            Ok(map) => map,
            Err(_) => return Vec::new(),
        };

        let mut ifaces: Vec<IfaceStats> = dev_status
            .into_iter()
            .map(|(name, status)| IfaceStats {
                name,
                rx_bytes: status.recv_bytes,
                tx_bytes: status.sent_bytes,
                rx_packets: status.recv_packets,
                tx_packets: status.sent_packets,
                rx_errors: status.recv_errs,
                tx_errors: status.sent_errs,
                rx_dropped: status.recv_drop,
                tx_dropped: status.sent_drop,
            })
            .collect();

        ifaces.sort_by(|a, b| a.name.cmp(&b.name));
        ifaces
    }

    fn compute_rates(
        &self,
        current: &[IfaceStats],
        now: Instant,
    ) -> HashMap<String, IfaceRate> {
        let mut rates = HashMap::new();

        let (Some(prev), Some(prev_time)) = (&self.prev_stats, &self.prev_time) else {
            return rates;
        };

        let elapsed = now.duration_since(*prev_time).as_secs_f64();
        if elapsed <= 0.0 {
            return rates;
        }

        let prev_map: HashMap<&str, &IfaceStats> =
            prev.iter().map(|s| (s.name.as_str(), s)).collect();

        for iface in current {
            if let Some(prev_iface) = prev_map.get(iface.name.as_str()) {
                let rx_diff = iface.rx_bytes.saturating_sub(prev_iface.rx_bytes) as f64;
                let tx_diff = iface.tx_bytes.saturating_sub(prev_iface.tx_bytes) as f64;
                rates.insert(
                    iface.name.clone(),
                    IfaceRate {
                        rx_bps: rx_diff / elapsed,
                        tx_bps: tx_diff / elapsed,
                    },
                );
            }
        }

        rates
    }

    fn format_addr(entry_addr: &std::net::SocketAddr) -> String {
        let ip = entry_addr.ip();
        let port = entry_addr.port();
        let port_str = if port == 0 { "*".to_string() } else { port.to_string() };

        match ip {
            std::net::IpAddr::V4(v4) => {
                let host = if v4.is_unspecified() { "*".to_string() } else { v4.to_string() };
                format!("{host}:{port_str}")
            }
            std::net::IpAddr::V6(v6) => {
                let host = if v6.is_unspecified() { "*".to_string() } else { v6.to_string() };
                format!("[{host}]:{port_str}")
            }
        }
    }

    fn tcp_state_str(state: &TcpState) -> &'static str {
        match state {
            TcpState::Established => "ESTABLISHED",
            TcpState::SynSent => "SYN_SENT",
            TcpState::SynRecv => "SYN_RECV",
            TcpState::FinWait1 => "FIN_WAIT1",
            TcpState::FinWait2 => "FIN_WAIT2",
            TcpState::TimeWait => "TIME_WAIT",
            TcpState::Close => "CLOSE",
            TcpState::CloseWait => "CLOSE_WAIT",
            TcpState::LastAck => "LAST_ACK",
            TcpState::Listen => "LISTEN",
            TcpState::Closing => "CLOSING",
            TcpState::NewSynRecv => "NEW_SYN_RECV",
            // All states covered above; no wildcard needed.
        }
    }

    fn convert_tcp(entry: &TcpNetEntry) -> TcpConn {
        TcpConn {
            local_addr: Self::format_addr(&entry.local_address),
            remote_addr: Self::format_addr(&entry.remote_address),
            state: Self::tcp_state_str(&entry.state).to_string(),
            uid: entry.uid,
            inode: entry.inode,
        }
    }

    fn convert_udp(entry: &UdpNetEntry) -> UdpSocket {
        UdpSocket {
            local_addr: Self::format_addr(&entry.local_address),
            remote_addr: Self::format_addr(&entry.remote_address),
            uid: entry.uid,
            inode: entry.inode,
        }
    }

    fn read_tcp() -> Vec<TcpConn> {
        let mut conns = Vec::new();

        if let Ok(entries) = procfs::net::tcp() {
            for entry in &entries {
                conns.push(Self::convert_tcp(entry));
            }
        }
        if let Ok(entries) = procfs::net::tcp6() {
            for entry in &entries {
                conns.push(Self::convert_tcp(entry));
            }
        }

        conns
    }

    fn read_udp() -> Vec<UdpSocket> {
        let mut sockets = Vec::new();

        if let Ok(entries) = procfs::net::udp() {
            for entry in &entries {
                sockets.push(Self::convert_udp(entry));
            }
        }
        if let Ok(entries) = procfs::net::udp6() {
            for entry in &entries {
                sockets.push(Self::convert_udp(entry));
            }
        }

        sockets
    }
}

/// Format bytes into a human-readable string (B, KB, MB, GB, TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Format bytes-per-second into a human-readable rate string.
pub fn format_rate(bps: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bps >= GB {
        format!("{:.2} GB/s", bps / GB)
    } else if bps >= MB {
        format!("{:.2} MB/s", bps / MB)
    } else if bps >= KB {
        format!("{:.2} KB/s", bps / KB)
    } else {
        format!("{:.0} B/s", bps)
    }
}
