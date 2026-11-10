# Rust MCP server

`scanner-mcp` exposes the scanner's typed tools over stdio and calls the same
Rust HTTP service used by Rune. Install the matching native distribution and
make `scanner-mcp` available on the MCP host's PATH, or configure its absolute
installed filename. It does not start the service or acquire a second scanner.

Set `SCANNER_TOKEN_FILE` to the absolute private credential filename in the MCP
host environment, or supply `--token-file /absolute/private/token` in its args.
The file contains the credential; do not place the token value in MCP configuration.
`--endpoint` defaults to `http://127.0.0.1:8171`. The repository `.mcp.json` exposes the full scanner tool set, preserving the
previous configuration. Add `--read-only` to restrict a host to inspection tools. The process logs errors to stderr and reserves stdout for MCP messages.

```sh
scanner-mcp --token-file /absolute/install/private/token --read-only
```

The service enforces authentication, ownership, source verification and mutation
validation independently of the MCP server. See [portable installation](portable-install.md)
and the schemas in `crates/scanner-mcp/src` for the available tools.
