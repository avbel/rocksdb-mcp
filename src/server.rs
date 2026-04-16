use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::{
    db::{Database, GetError},
    encoding::{self, Encoding, EncodingError},
};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetValueArgs {
    /// Column family name, exactly as returned by `list_column_families`.
    /// Case-sensitive. The always-present default CF is literally `"default"`.
    pub column_family: String,

    /// The lookup key, encoded per `key_encoding`. Examples: utf8 `"users/42"`,
    /// hex `"a1b2c3"`, base64 `"obLD"`.
    pub key: String,

    /// How to decode `key` into raw bytes. One of: `"utf8"`, `"hex"`, `"base64"`.
    /// Default: `"utf8"`. Use `"hex"` or `"base64"` for non-text keys.
    #[serde(default)]
    pub key_encoding: Encoding,

    /// How to encode the returned value bytes in the response. One of:
    /// `"utf8"`, `"hex"`, `"base64"`. Default: `"utf8"`. Using `"utf8"` on
    /// non-UTF-8 bytes returns an error; prefer `"base64"` for unknown or
    /// binary payloads.
    #[serde(default)]
    pub value_encoding: Encoding,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum GetValueResult {
    Found {
        found: bool,
        value: String,
        value_encoding: Encoding,
    },
    Missing {
        found: bool,
    },
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListColumnFamiliesResult {
    pub column_families: Vec<String>,
}

#[derive(Clone)]
pub struct RocksDbServer {
    db: Arc<Database>,
    #[expect(dead_code, reason = "consumed by #[tool_handler] macro via ToolRouter")]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl RocksDbServer {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Lists every column family in the currently-open RocksDB database. \
Call this first when you do not know the exact column-family name required by the `get_value` tool. \
Returns the full set of CF names as a JSON array; there is no pagination. \
Column families are RocksDB's equivalent of tables / namespaces, and the `default` CF always exists. \
Order reflects RocksDB's own enumeration and is not guaranteed to be alphabetical."
    )]
    async fn list_column_families(&self) -> Result<CallToolResult, McpError> {
        let result = ListColumnFamiliesResult {
            column_families: self.db.column_families().to_vec(),
        };
        json_result(&result)
    }

    #[tool(
        description = "Fetches the value for a single key from a specific column family. \
This is a point lookup, not a range scan or prefix search. \
If the key does not exist, the tool returns `{\"found\": false}` — that is NOT an error. \
Binary keys and values are fully supported via the `key_encoding` and `value_encoding` fields (`\"hex\"` or `\"base64\"`). \
Use `\"utf8\"` only when you know the bytes are valid UTF-8 text; otherwise the call will error and point you at `\"base64\"`. \
If you don't know the exact column-family name, call `list_column_families` first."
    )]
    async fn get_value(
        &self,
        Parameters(args): Parameters<GetValueArgs>,
    ) -> Result<CallToolResult, McpError> {
        let key_bytes = encoding::decode("key", &args.key, args.key_encoding).map_err(map_enc)?;

        let value = self
            .db
            .get(&args.column_family, key_bytes.as_ref())
            .map_err(map_get)?;

        let Some(bytes) = value else {
            return json_result(&GetValueResult::Missing { found: false });
        };

        let encoded = encoding::encode("value", &bytes, args.value_encoding).map_err(map_enc)?;
        json_result(&GetValueResult::Found {
            found: true,
            value: encoded,
            value_encoding: args.value_encoding,
        })
    }
}

#[tool_handler]
impl ServerHandler for RocksDbServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Read-only RocksDB MCP server. Two tools: `list_column_families` enumerates \
column families; `get_value` performs a single point lookup within a CF. \
Call `list_column_families` first if you don't know the CF name. For \
non-text keys or values set `key_encoding` / `value_encoding` to `\"hex\"` \
or `\"base64\"`.",
            )
    }
}

fn json_result<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string(value).map_err(|e| {
        McpError::internal_error(format!("failed to serialize tool response: {e}"), None)
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_enc(e: EncodingError) -> McpError {
    McpError::invalid_params(e.to_string(), None)
}

fn map_get(e: GetError) -> McpError {
    match e {
        GetError::UnknownColumnFamily(_) => McpError::invalid_params(e.to_string(), None),
        GetError::RocksDb(_) => McpError::internal_error(e.to_string(), None),
    }
}
