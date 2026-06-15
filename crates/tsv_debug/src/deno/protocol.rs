//! Wire protocol types for Deno sidecar communication
//!
//! JSON-lines protocol over stdio. Matches the format expected by sidecar.ts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request sent to the Deno sidecar (JSON-lines over stdin)
#[derive(Debug, Serialize)]
pub struct WireRequest {
    pub id: u64,
    pub tool: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Value>,
}

/// Response from the Deno sidecar (JSON-lines over stdout)
#[derive(Debug, Deserialize)]
pub struct WireResponse {
    /// Request ID. -1 indicates a malformed request error (couldn't parse request JSON)
    pub id: i64,
    pub ok: bool,
    pub output: Option<Value>,
    pub error: Option<String>,
}
