# Provider Registry

Provider registry entries live under `registry/providers/<provider-id>/`.

Every registry PR must include these files:

- `registry/providers/<provider-id>/manifest.json`
- `registry/providers/<provider-id>/README.md`
- `registry/providers/<provider-id>/examples/basic.task.json`

Run the registry gate before opening a PR:

```bash
just registry-check
```

The gate validates manifests, required files, duplicate provider IDs, directory
name consistency, and schema freshness for
`schemas/provider-manifest.schema.json`.

Capabilities in a manifest are declarations only. They do not grant
authorization, directory access, provider execution rights, or access to secrets.
Future runtime phases must still request and enforce permissions separately.

Providers that send prompts, files, repository context, or code to a vendor or
remote service must disclose that behavior in `manifest.json` through
`capabilities.remote_execution` and `security.sends_code_to_vendor`, and should
summarize it in the provider README.
