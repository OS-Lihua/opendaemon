use std::net::IpAddr;

use clap::error::ErrorKind;

use crate::{
    api::AppState,
    cli::{Cli, Command},
    config::DaemonConfig,
};

#[test]
fn cli_parser_accepts_daemon() {
    let cli = Cli::parse_from_for_test(["opendaemon", "daemon"]).unwrap();

    assert!(matches!(cli.command_for_test(), Command::Daemon(_)));
}

#[test]
fn cli_parser_accepts_registry_check() {
    let cli = Cli::parse_from_for_test(["opendaemon", "registry-check"]).unwrap();

    assert!(matches!(cli.command_for_test(), Command::RegistryCheck));
}

#[test]
fn cli_parser_uses_default_daemon_bind_address() {
    let cli = Cli::parse_from_for_test(["opendaemon", "daemon"]).unwrap();
    let config = cli.command_for_test().daemon_args_for_test().config();

    assert_eq!(
        config,
        DaemonConfig::new(IpAddr::from([127, 0, 0, 1]), 19514)
    );
}

#[test]
fn cli_parser_accepts_host_and_port_overrides() {
    let cli = Cli::parse_from_for_test([
        "opendaemon",
        "daemon",
        "--host",
        "127.0.0.2",
        "--port",
        "49152",
    ])
    .unwrap();
    let config = cli.command_for_test().daemon_args_for_test().config();

    assert_eq!(
        config,
        DaemonConfig::new(IpAddr::from([127, 0, 0, 2]), 49152)
    );
}

#[test]
fn cli_parser_accepts_ephemeral_port() {
    let cli = Cli::parse_from_for_test(["opendaemon", "daemon", "--port", "0"]).unwrap();
    let config = cli.command_for_test().daemon_args_for_test().config();

    assert_eq!(config.port, 0);
}

#[test]
fn cli_parser_rejects_invalid_arguments() {
    let error =
        Cli::parse_from_for_test(["opendaemon", "daemon", "--port", "invalid"]).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::ValueValidation);
}

#[tokio::test]
async fn app_state_from_env_loads_bootstrap_token() {
    let _guard = crate::tests::process_env_test_guard().await;
    let key = "OPENDAEMON_BOOTSTRAP_TOKEN";
    let previous = std::env::var_os(key);
    unsafe {
        std::env::set_var(key, "phase8-bootstrap-test-token");
    }

    let state = AppState::from_env();
    assert_eq!(
        state.auth_config().bootstrap_token.as_deref(),
        Some("phase8-bootstrap-test-token")
    );

    match previous {
        Some(value) => unsafe {
            std::env::set_var(key, value);
        },
        None => unsafe {
            std::env::remove_var(key);
        },
    }
}
