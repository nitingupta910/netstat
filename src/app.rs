use crate::network::{NetworkCollector, NetworkData};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Interfaces,
    Connections,
    Bandwidth,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Interfaces, Tab::Connections, Tab::Bandwidth];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Interfaces => "Interfaces",
            Tab::Connections => "Connections",
            Tab::Bandwidth => "Bandwidth",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Tab::Interfaces => 0,
            Tab::Connections => 1,
            Tab::Bandwidth => 2,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnFilter {
    All,
    Tcp,
    Udp,
}

pub struct App {
    pub running: bool,
    pub tab: Tab,
    pub data: NetworkData,
    pub conn_filter: ConnFilter,
    pub conn_scroll: usize,
    pub iface_scroll: usize,
    pub selected_iface: usize,
    collector: NetworkCollector,
}

impl App {
    pub fn new() -> Self {
        let mut collector = NetworkCollector::new();
        let data = collector.collect();
        Self {
            running: true,
            tab: Tab::Interfaces,
            data,
            conn_filter: ConnFilter::All,
            conn_scroll: 0,
            iface_scroll: 0,
            selected_iface: 0,
            collector,
        }
    }

    pub fn tick(&mut self) {
        self.data = self.collector.collect();
    }

    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Interfaces => Tab::Connections,
            Tab::Connections => Tab::Bandwidth,
            Tab::Bandwidth => Tab::Interfaces,
        };
    }

    pub fn prev_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Interfaces => Tab::Bandwidth,
            Tab::Connections => Tab::Interfaces,
            Tab::Bandwidth => Tab::Connections,
        };
    }

    pub fn scroll_down(&mut self) {
        match self.tab {
            Tab::Interfaces => {
                if !self.data.interfaces.is_empty() {
                    self.iface_scroll = (self.iface_scroll + 1).min(self.data.interfaces.len() - 1);
                }
            }
            Tab::Connections => {
                let max = self.conn_count().saturating_sub(1);
                self.conn_scroll = self.conn_scroll.saturating_add(1).min(max);
            }
            Tab::Bandwidth => {
                if !self.data.interfaces.is_empty() {
                    self.selected_iface =
                        (self.selected_iface + 1) % self.data.interfaces.len();
                }
            }
        }
    }

    pub fn scroll_up(&mut self) {
        match self.tab {
            Tab::Interfaces => {
                self.iface_scroll = self.iface_scroll.saturating_sub(1);
            }
            Tab::Connections => {
                self.conn_scroll = self.conn_scroll.saturating_sub(1);
            }
            Tab::Bandwidth => {
                if !self.data.interfaces.is_empty() {
                    self.selected_iface = if self.selected_iface == 0 {
                        self.data.interfaces.len() - 1
                    } else {
                        self.selected_iface - 1
                    };
                }
            }
        }
    }

    fn conn_count(&self) -> usize {
        match self.conn_filter {
            ConnFilter::All => self.data.tcp_conns.len() + self.data.udp_sockets.len(),
            ConnFilter::Tcp => self.data.tcp_conns.len(),
            ConnFilter::Udp => self.data.udp_sockets.len(),
        }
    }

    pub fn cycle_conn_filter(&mut self) {
        self.conn_filter = match self.conn_filter {
            ConnFilter::All => ConnFilter::Tcp,
            ConnFilter::Tcp => ConnFilter::Udp,
            ConnFilter::Udp => ConnFilter::All,
        };
        self.conn_scroll = 0;
    }
}
