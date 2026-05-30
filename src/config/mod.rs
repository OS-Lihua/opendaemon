use std::{
    collections::BTreeMap,
    ffi::OsString,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

pub const DEFAULT_DAEMON_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
pub const DEFAULT_DAEMON_PORT: u16 = 19514;
pub const DEFAULT_RUNTIME_DETECTION_TIMEOUT: Duration = Duration::from_secs(2);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDetectionConfig {
    pub timeout: Duration,
    pub environment: RuntimeEnvironment,
}

impl RuntimeDetectionConfig {
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn with_environment(mut self, environment: RuntimeEnvironment) -> Self {
        self.environment = environment;
        self
    }
}

impl Default for RuntimeDetectionConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_RUNTIME_DETECTION_TIMEOUT,
            environment: RuntimeEnvironment::system(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEnvironment {
    System,
    Map(BTreeMap<String, OsString>),
}

impl RuntimeEnvironment {
    #[must_use]
    pub const fn system() -> Self {
        Self::System
    }

    pub fn from_vars<I, K, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<OsString>,
    {
        Self::Map(
            vars.into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        )
    }

    #[must_use]
    pub fn var_os(&self, key: &str) -> Option<OsString> {
        match self {
            Self::System => std::env::var_os(key),
            Self::Map(vars) => vars.get(key).cloned(),
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<OsString> {
        self.var_os("PATH")
    }
}
