use crate::ipc::HostIpcClient;
use crate::mcp::protocol::{
    CallToolParams, Implementation, InitializeResult, JsonRpcRequest, JsonRpcResponse,
    ListToolsResult, ServerCapabilities, ToolsCapability,
};
use crate::mcp::tools::{execute_tool, get_tool_declarations};
use serde_json::json;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// Standards-compliant Model Context Protocol server exposing OmniScope capabilities over JSON-RPC 2.0.
#[derive(Clone)]
pub struct McpServer {
    pub ipc_client: HostIpcClient,
}

impl McpServer {
    pub fn new(ipc_client: HostIpcClient) -> Self {
        Self { ipc_client }
    }

    /// Processes a single JSON-RPC 2.0 request and returns the serialized response.
    ///
    /// Notifications (requests without an `id`) return `None` per standard JSON-RPC rules.
    pub async fn handle_request(
        &self,
        request: JsonRpcRequest,
        trace_id: u64,
    ) -> Option<JsonRpcResponse> {
        let id = request.id.clone();

        match request.method.as_str() {
            "initialize" => {
                let result = InitializeResult {
                    protocol_version: "2024-11-05".to_string(),
                    capabilities: ServerCapabilities {
                        tools: Some(ToolsCapability { list_changed: Some(false) }),
                    },
                    server_info: Implementation {
                        name: "omniscope".to_string(),
                        version: "0.1.0".to_string(),
                    },
                };
                Some(JsonRpcResponse::success(id, json!(result)))
            }

            "notifications/initialized" => {
                // Client initialization handshake complete
                None
            }

            "ping" => Some(JsonRpcResponse::success(id, json!({}))),

            "tools/list" => {
                let tools = get_tool_declarations();
                Some(JsonRpcResponse::success(id, json!(ListToolsResult { tools })))
            }

            "tools/call" => {
                let params = match request.params {
                    Some(val) => match serde_json::from_value::<CallToolParams>(val) {
                        Ok(p) => p,
                        Err(e) => {
                            return Some(JsonRpcResponse::error(
                                id,
                                -32602,
                                format!("Invalid tool call parameters: {}", e),
                                None,
                            ));
                        }
                    },
                    None => {
                        return Some(JsonRpcResponse::error(
                            id,
                            -32602,
                            "Missing params for tools/call",
                            None,
                        ));
                    }
                };

                match execute_tool(&self.ipc_client, &params.name, params.arguments, trace_id).await {
                    Ok(result) => Some(JsonRpcResponse::success(id, json!(result))),
                    Err(e) => Some(JsonRpcResponse::error(
                        id,
                        -32603,
                        format!("Tool execution error: {}", e),
                        None,
                    )),
                }
            }

            unknown => Some(JsonRpcResponse::error(
                id,
                -32601,
                format!("Method '{}' not found", unknown),
                None,
            )),
        }
    }

    /// Runs a line-delimited stdio server loop until stream EOF.
    pub async fn run_stream_loop<R, W>(&self, reader: R, mut writer: W) -> Result<(), std::io::Error>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut lines = reader.lines();
        let mut trace_counter = 1u64;

        while let Some(line) = lines.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(line) {
                Ok(req) => req,
                Err(e) => {
                    let err_resp = JsonRpcResponse::error(
                        None,
                        -32700,
                        format!("Parse error: {}", e),
                        None,
                    );
                    let mut bytes = serde_json::to_vec(&err_resp)?;
                    bytes.push(b'\n');
                    writer.write_all(&bytes).await?;
                    writer.flush().await?;
                    continue;
                }
            };

            trace_counter += 1;
            if let Some(response) = self.handle_request(request, trace_counter).await {
                let mut bytes = serde_json::to_vec(&response)?;
                bytes.push(b'\n');
                writer.write_all(&bytes).await?;
                writer.flush().await?;
            }
        }

        Ok(())
    }
}
