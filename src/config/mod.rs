use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const DEFAULT_DAEMON_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
pub const DEFAULT_DAEMON_PORT: u16 = 19514;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonConfig {
    pub host: IpAddr,
    pub port: u16,
}

impl DaemonConfig {
    #[must_use]
    pub const fn new(host: IpAddr, port: u16) -> Self {
        Self { host, port }
    }

    #[must_use]
    pub const fn socket_addr(self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::new(DEFAULT_DAEMON_HOST, DEFAULT_DAEMON_PORT)
    }
}
