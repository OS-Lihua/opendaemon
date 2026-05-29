# OpenDaemon

Local daemon foundation for coordinating OpenDaemon providers and tasks.

### DEV

Phase 0 provides the project identity, CLI entrypoint, daemon HTTP server, and
health endpoint.

#### Tests

Unit and integration-style tests live in the Rust crate.

E2E tests should live in the `e2e` directory and use `uv` plus Python.
