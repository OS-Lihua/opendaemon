# Local API Authentication

OpenDaemon's Phase 8 authentication model protects the local daemon boundary. It is not a hardened internet-facing deployment model.

- The daemon still defaults to loopback binding.
- `OPENDAEMON_BOOTSTRAP_TOKEN` is a local operator credential for product-management routes only.
- Product API tokens are bearer credentials scoped to one product and a fixed scope set.
- Exposing the daemon through a reverse proxy or binding it on a non-loopback interface expands the attack surface substantially.
- Bootstrap tokens and product tokens should be treated like local machine credentials. Anyone who can read them can act through the daemon within their granted scope.
- Remote control-plane support belongs to a later phase with different trust boundaries and token handling.

Operational guidance:

- Keep the daemon on loopback unless you are deliberately accepting remote-access risk.
- Do not pass bootstrap or product tokens through task payloads, provider config, or child-process environment variables.
- Rotate the bootstrap token by changing daemon configuration and restarting the daemon.
- Revoke and reissue product tokens instead of trying to extend them in place.
