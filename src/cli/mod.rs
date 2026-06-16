use std::{future::Future, net::IpAddr};

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use tokio::{net::TcpListener, signal};

use crate::{
    api,
    config::{DEFAULT_DAEMON_HOST, DEFAULT_DAEMON_PORT, DaemonConfig},
    registry,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "OpenDaemon local daemon")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Daemon(DaemonArgs),
    RegistryCheck,
}

#[derive(Debug, Args)]
pub struct DaemonArgs {
    #[arg(long, env = "OPENDAEMON_DAEMON_HOST", default_value_t = DEFAULT_DAEMON_HOST)]
    host: IpAddr,
    #[arg(long, env = "OPENDAEMON_DAEMON_PORT", default_value_t = DEFAULT_DAEMON_PORT)]
    port: u16,
}

impl DaemonArgs {
    #[must_use]
    pub const fn config(&self) -> DaemonConfig {
        DaemonConfig::new(self.host, self.port)
    }
}

pub async fn run() -> anyhow::Result<()> {
    run_with_args(Cli::parse()).await
}

pub async fn run_with_args(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Daemon(args) => run_daemon_until_shutdown(args.config(), shutdown_signal()).await,
        Command::RegistryCheck => registry::check_default_registry(),
    }
}

pub async fn run_daemon_until_shutdown<S>(
    config: DaemonConfig,
    shutdown_signal: S,
) -> anyhow::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(config.socket_addr())
        .await
        .with_context(|| format!("failed to bind daemon to {}", config.socket_addr()))?;
    let bound_addr = listener
        .local_addr()
        .context("failed to read daemon bound address")?;

    tracing::info!(%bound_addr, "opendaemon daemon listening");

    let state = api::AppState::from_env();
    let control_plane = crate::control_plane::client::spawn_if_enabled(state.clone())?;

    axum::serve(listener, api::router_with_state(state))
        .with_graceful_shutdown(async move {
            shutdown_signal.await;
            if let Some(handle) = control_plane {
                handle.abort();
                let _ = handle.await;
            }
        })
        .await
        .context("daemon HTTP server failed")
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for shutdown signal");
    }
}

#[cfg(test)]
impl Cli {
    pub fn parse_from_for_test<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args)
    }
}

#[cfg(test)]
impl Command {
    pub const fn daemon_args_for_test(&self) -> &DaemonArgs {
        match self {
            Self::Daemon(args) => args,
            Self::RegistryCheck => panic!("registry-check does not have daemon args"),
        }
    }
}

#[cfg(test)]
impl Cli {
    pub const fn command_for_test(&self) -> &Command {
        &self.command
    }
}
