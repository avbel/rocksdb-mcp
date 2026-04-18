# rocksdb-mcp

A Streamable-HTTP [MCP](https://modelcontextprotocol.io/) server that exposes a
**read-only** view of a RocksDB database.

- Two tools: `list_column_families`, `get_value`
- Two modes: read-only snapshot (default) or read-only secondary with auto-refresh
- Optional bearer-token auth
- Single static binary

## Install

### Homebrew

```sh
brew install avbel/tap/rocksdb-mcp
```

### Prebuilt binaries

Download the archive for your platform from
[GitHub Releases](https://github.com/avbel/rocksdb-mcp/releases):

- Linux x86_64 — `rocksdb-mcp-<version>-x86_64-unknown-linux-gnu.tar.gz`
- macOS arm64 — `rocksdb-mcp-<version>-aarch64-apple-darwin.zip`
- Windows x86_64 — `rocksdb-mcp-<version>-x86_64-pc-windows-msvc.zip`

### From source

Build deps: a C++ toolchain plus `clang` / `libclang` (required by the
[`rocksdb`](https://crates.io/crates/rocksdb) crate's bindgen step). On Debian /
Ubuntu: `sudo apt-get install -y clang libclang-dev`.

```sh
cargo install --path . --locked
```

## Quickstart

Snapshot mode (default — opens the DB read-only, point-in-time view at open):

```sh
rocksdb-mcp --db-path /var/lib/mydb
```

Secondary mode (periodically catches up with a primary process that keeps
writing):

```sh
rocksdb-mcp \
  --mode secondary \
  --db-path /var/lib/mydb \
  --secondary-path /tmp/rocksdb-mcp \
  --refresh-interval 2s
```

With bearer-token auth:

```sh
MCP_API_TOKEN=s3cret rocksdb-mcp --db-path /var/lib/mydb
```

Show the version:

```sh
rocksdb-mcp --version
```

## Configuration

Every flag has a matching environment variable; the CLI takes precedence.

| Flag | Env | Default | Notes |
|---|---|---|---|
| `--db-path` | `ROCKSDB_PATH` | — | **Required.** Path to the primary DB directory. |
| `--mode` | `ROCKSDB_MODE` | `snapshot` | `snapshot` or `secondary`. |
| `--secondary-path` | `ROCKSDB_SECONDARY_PATH` | — | Required when `--mode=secondary`. Writable directory for the secondary instance's own state. |
| `--refresh-interval` | `ROCKSDB_REFRESH_INTERVAL` | `5s` | Duration (`1s`, `500ms`, `2m`…). Only used in secondary mode — the server calls `try_catch_up_with_primary()` on this cadence. |
| `--host` | `MCP_HOST` | `127.0.0.1` | Bind address. |
| `--port` | `MCP_PORT` | `8080` | Bind port. |
| `--api-token` | `MCP_API_TOKEN` | _unset_ | When set, clients must send `Authorization: Bearer <token>`. |

The MCP endpoint is mounted at **`/mcp`** (e.g. `http://127.0.0.1:8080/mcp`).

## Modes

**Snapshot** (`--mode snapshot`, default) opens the database with RocksDB's
read-only API. No WAL replay, no compactions, no locking conflict with another
process — but the view is *point-in-time* at the moment the server opened the
DB. Restart to pick up new writes.

**Secondary** (`--mode secondary`) opens the database as a RocksDB secondary
instance. A background task calls `try_catch_up_with_primary()` every
`--refresh-interval`, replaying the primary's WAL tail. Use this when another
process is writing to the DB and you want the MCP server to see new values
with at most `refresh-interval` lag.

The tradeoff is staleness vs. cost: a smaller interval reduces lag but
replays WAL more often. `1s`–`5s` is usually fine.

## Auth

Set `--api-token` (or `MCP_API_TOKEN`). Clients must send:

```
Authorization: Bearer <token>
```

Missing or wrong token → `401 Unauthorized`. Without a token configured, the
server is unauthenticated — put it behind a reverse proxy or bind to
`127.0.0.1` only.

## MCP tools

### `list_column_families`

Returns every column family that exists in the DB.

```sh
curl -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call",
       "params":{"name":"list_column_families","arguments":{}}}'
```

### `get_value`

Point lookup for a single key in a specific column family. Binary keys and
values supported via `key_encoding` / `value_encoding` = `"hex"` or `"base64"`.

```sh
curl -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"get_value","arguments":{
         "column_family":"default",
         "key":"users/42",
         "value_encoding":"base64"
       }}}'
```

Responses use `{"found": true, "value": "...", "value_encoding": "..."}` on
hit, `{"found": false}` on miss, and a standard JSON-RPC error otherwise. Use
`base64` when you're not sure the value is UTF-8.

## Connecting from an MCP client

Claude Code (`~/.claude.json` or equivalent):

```json
{
  "mcpServers": {
    "rocksdb": {
      "type": "http",
      "url": "http://127.0.0.1:8080/mcp",
      "headers": { "Authorization": "Bearer s3cret" }
    }
  }
}
```

Any Streamable-HTTP-capable MCP client (e.g. MCP Inspector) works the same way.

## Development

```sh
cargo fmt --all           # format
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run -- --db-path /tmp/mydb
```

CI runs `fmt`, `clippy -D warnings`, and the test suite on every push and PR
(see `.github/workflows/ci.yml`).

### Seeding a test DB

```sh
cargo run --example seed -- /tmp/mydb
```

Creates a small RocksDB with two column families (`default`, `meta`) and a few
keys for experimentation.

## Releasing

Release is fully automated by `.github/workflows/release.yml`:

1. Bump `version` in `Cargo.toml`, commit, and tag `vX.Y.Z`.
2. Push the tag. CI builds Linux x86_64, macOS arm64, and Windows x86_64,
   publishes a GitHub Release with the archives, and pushes an updated
   formula to `avbel/homebrew-tap` when tap credentials are configured.

The Homebrew job needs a repo secret named `HOMEBREW_TAP_TOKEN` (a PAT with
`contents:write` on the tap repository). Without that secret, the release still
publishes the GitHub Release artifacts and skips the tap update.

## License

[MIT](LICENSE)
