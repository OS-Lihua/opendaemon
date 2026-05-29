use std::{
    io::{Read, Write},
    net::{IpAddr, TcpStream},
    time::Duration,
};

use clap::error::ErrorKind;
use tokio::{net::TcpListener, sync::oneshot, time::timeout};

use crate::{
    api::{HealthResponse, health, router},
    cli::{Cli, Command, run_daemon_until_shutdown},
    config::DaemonConfig,
};

#[test]
fn cli_parser_accepts_daemon() {
    let cli = Cli::parse_from_for_test(["opendaemon", "daemon"]).unwrap();

    assert!(matches!(cli.command_for_test(), Command::Daemon(_)));
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
async fn health_handler_returns_stable_json() {
    let response = health().await.0;

    assert_eq!(
        response,
        HealthResponse {
            status: "ok",
            service: "opendaemon",
            version: env!("CARGO_PKG_VERSION"),
        }
    );
}

#[tokio::test]
async fn router_serves_health_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router())
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    let response = tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        String::from_utf8(response).unwrap()
    })
    .await
    .unwrap();

    shutdown_tx.send(()).unwrap();
    server.await.unwrap();

    let expected = format!(
        r#"{{"status":"ok","service":"opendaemon","version":"{}"}}"#,
        env!("CARGO_PKG_VERSION")
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.ends_with(&expected));
}

#[tokio::test]
async fn daemon_binds_ephemeral_port_and_stops_on_shutdown_signal() {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(run_daemon_until_shutdown(
        DaemonConfig::new(IpAddr::from([127, 0, 0, 1]), 0),
        async {
            let _ = shutdown_rx.await;
        },
    ));

    shutdown_tx.send(()).unwrap();

    let result = timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    result.unwrap();
}
