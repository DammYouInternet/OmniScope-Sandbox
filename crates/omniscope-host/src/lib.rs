pub mod ipc;
pub mod mcp;
pub mod vm;

pub use ipc::{HostIpcClient, IpcError};
pub use mcp::{
    CallToolParams, CallToolResult, InitializeResult, JsonRpcRequest, JsonRpcResponse,
    ListToolsResult, McpServer, Tool, ToolContent,
};
pub use vm::{
    build_qemu_args, HypervisorController, MockHypervisorController, RealQmpController,
    StealthConfig, VmConfig, VmError, VmManager,
};

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::commands::{GuestCommand, GuestResponse};
    use omniscope_core::events::{ButtonAction, ImageFormat, MouseButton, TelemetrySource};
    use omniscope_guest::actuation::MockInputDriver;
    use omniscope_guest::ipc::GuestDispatcher;
    use omniscope_guest::perception::{MockFrameGrabber, MockUiTreeWalker};
    use omniscope_guest::telemetry::TapManager;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;

    fn setup_test_ipc_pair() -> (HostIpcClient, tokio::task::JoinHandle<()>) {
        let (host_stream, guest_stream) = tokio::io::duplex(64 * 1024);

        let input_driver = Arc::new(MockInputDriver::new(1920, 1080));
        let frame_grabber = Arc::new(MockFrameGrabber::new(1280, 720));
        let ui_walker = Arc::new(MockUiTreeWalker::new());
        let tap_manager = Arc::new(TapManager::default());

        let dispatcher = Arc::new(GuestDispatcher::new(
            input_driver,
            frame_grabber,
            ui_walker,
            tap_manager,
        ));

        let guest_handle = tokio::spawn(async move {
            let _ = dispatcher.run_loop(guest_stream).await;
        });

        let host_client = HostIpcClient::new(host_stream);
        (host_client, guest_handle)
    }

    // -----------------------------------------------------------------------
    // Chunk 5: Host-Guest IPC Tests
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_ipc_loopback_ping_pong() {
        let (client, _handle) = setup_test_ipc_pair();

        let resp = client
            .send_command(
                GuestCommand::Ping {
                    timestamp_ms: 0xCAFEBABE,
                },
                1,
                Duration::from_secs(2),
            )
            .await
            .expect("Ping command must succeed");

        assert_eq!(
            resp,
            GuestResponse::Pong {
                timestamp_ms: 0xCAFEBABE,
            }
        );
    }

    #[tokio::test]
    async fn test_ipc_loopback_capture_screen() {
        let (client, _handle) = setup_test_ipc_pair();

        let resp = client
            .send_command(
                GuestCommand::CaptureScreen {
                    format: ImageFormat::WebP,
                    quality: 85,
                    crop_active_window: false,
                },
                2,
                Duration::from_secs(2),
            )
            .await
            .expect("Capture screen must succeed");

        match resp {
            GuestResponse::ScreenFrame {
                width,
                height,
                format,
                data,
            } => {
                assert_eq!(width, 1280);
                assert_eq!(height, 720);
                assert_eq!(format, ImageFormat::WebP);
                assert!(!data.is_empty());
            }
            other => panic!("Expected ScreenFrame response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_ipc_loopback_accessibility_tree() {
        let (client, _handle) = setup_test_ipc_pair();

        let resp = client
            .send_command(
                GuestCommand::GetAccessibilityTree {
                    max_depth: 4,
                    root_hwnd: None,
                },
                3,
                Duration::from_secs(2),
            )
            .await
            .expect("Get accessibility tree must succeed");

        match resp {
            GuestResponse::AccessibilityTree { root } => {
                assert_eq!(root.name, "Desktop");
                assert!(!root.children.is_empty());
            }
            other => panic!("Expected AccessibilityTree response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_ipc_loopback_actuation_and_telemetry() {
        let (client, _handle) = setup_test_ipc_pair();

        let move_resp = client
            .send_command(
                GuestCommand::InjectMouseMove {
                    target_x: 640,
                    target_y: 480,
                    duration_ms: 50,
                    use_bezier_curve: true,
                },
                4,
                Duration::from_secs(2),
            )
            .await
            .expect("InjectMouseMove must succeed");

        assert_eq!(move_resp, GuestResponse::ActuationCompleted { success: true });

        let click_resp = client
            .send_command(
                GuestCommand::InjectMouseButton {
                    button: MouseButton::Left,
                    action: ButtonAction::Click,
                },
                5,
                Duration::from_secs(2),
            )
            .await
            .expect("InjectMouseButton must succeed");

        assert_eq!(click_resp, GuestResponse::ActuationCompleted { success: true });

        let query_resp = client
            .send_command(
                GuestCommand::QueryTelemetrySlice {
                    source: TelemetrySource::FileSystem,
                    pid_filter: None,
                    string_filter: None,
                    limit: 10,
                },
                6,
                Duration::from_secs(2),
            )
            .await
            .expect("QueryTelemetrySlice must succeed");

        match query_resp {
            GuestResponse::TelemetrySlice { source, events, .. } => {
                assert_eq!(source, TelemetrySource::FileSystem);
                assert_eq!(events.len(), 0);
            }
            other => panic!("Expected TelemetrySlice response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_ipc_concurrent_request_correlation() {
        let (client, _handle) = setup_test_ipc_pair();
        let mut handles = Vec::new();

        for i in 1..=15 {
            let client_clone = client.clone();
            handles.push(tokio::spawn(async move {
                let timestamp = i * 1000 + 42;
                let resp = client_clone
                    .send_command(
                        GuestCommand::Ping {
                            timestamp_ms: timestamp,
                        },
                        i,
                        Duration::from_secs(5),
                    )
                    .await
                    .expect("Concurrent ping must succeed");

                assert_eq!(resp, GuestResponse::Pong { timestamp_ms: timestamp });
            }));
        }

        for handle in handles {
            handle.await.expect("Task must complete without panic");
        }
    }

    // -----------------------------------------------------------------------
    // Chunk 6: Model Context Protocol (MCP) AI Server Tests
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_mcp_initialize_handshake() {
        let (client, _handle) = setup_test_ipc_pair();
        let server = McpServer::new(client);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: None,
        };

        let resp = server.handle_request(req, 1).await.expect("Must produce response");
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(json!(1)));

        let result = resp.result.expect("Must have result payload");
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "omniscope");
    }

    #[tokio::test]
    async fn test_mcp_tools_list_all_ten_tools() {
        let (client, _handle) = setup_test_ipc_pair();
        let server = McpServer::new(client);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };

        let resp = server.handle_request(req, 2).await.expect("Must produce response");
        let result = resp.result.expect("Must have result payload");
        let tools = result["tools"].as_array().expect("tools must be an array");

        assert_eq!(tools.len(), 10);
        let tool_names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();

        let expected_tools = [
            "sandbox_status",
            "sandbox_snapshot_create",
            "sandbox_snapshot_revert",
            "sandbox_launch_app",
            "sandbox_get_screen",
            "sandbox_get_ui_tree",
            "sandbox_click",
            "sandbox_type_text",
            "sandbox_query_telemetry",
            "sandbox_inspect_memory",
        ];

        for expected in &expected_tools {
            assert!(
                tool_names.contains(expected),
                "Missing expected tool: {}",
                expected
            );
        }
    }

    #[tokio::test]
    async fn test_mcp_tools_call_status_and_actions() {
        let (client, _handle) = setup_test_ipc_pair();
        let server = McpServer::new(client);

        // 1. Call sandbox_status
        let req_status = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(101)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "sandbox_status",
                "arguments": {}
            })),
        };

        let resp_status = server.handle_request(req_status, 101).await.unwrap();
        let result_status = resp_status.result.unwrap();
        let content_status = result_status["content"][0]["text"].as_str().unwrap();
        assert!(content_status.contains("OmniScope Sandbox Status"));
        assert!(content_status.contains("Active (QEMU/WHPX)"));

        // 2. Call sandbox_click
        let req_click = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(102)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "sandbox_click",
                "arguments": {
                    "x": 450,
                    "y": 300,
                    "button": "left"
                }
            })),
        };

        let resp_click = server.handle_request(req_click, 102).await.unwrap();
        let result_click = resp_click.result.unwrap();
        let content_click = result_click["content"][0]["text"].as_str().unwrap();
        assert!(content_click.contains("Clicked at coordinates (450, 300)"));

        // 3. Call sandbox_type_text
        let req_type = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(103)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "sandbox_type_text",
                "arguments": {
                    "text": "SecretLicenseKey123",
                    "press_enter": true
                }
            })),
        };

        let resp_type = server.handle_request(req_type, 103).await.unwrap();
        let result_type = resp_type.result.unwrap();
        let content_type = result_type["content"][0]["text"].as_str().unwrap();
        assert!(content_type.contains("Successfully synthesized keystrokes"));

        // 4. Call sandbox_query_telemetry
        let req_telemetry = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(104)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "sandbox_query_telemetry",
                "arguments": {
                    "source": "filesystem",
                    "limit": 5
                }
            })),
        };

        let resp_telemetry = server.handle_request(req_telemetry, 104).await.unwrap();
        let result_telemetry = resp_telemetry.result.unwrap();
        let content_telemetry = result_telemetry["content"][0]["text"].as_str().unwrap();
        assert!(content_telemetry.contains("Telemetry Query Slice"));
    }

    #[tokio::test]
    async fn test_mcp_stream_stdio_loop() {
        let (client, _handle) = setup_test_ipc_pair();
        let server = McpServer::new(client);

        let (client_io, server_io) = tokio::io::duplex(32 * 1024);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let (client_reader, mut client_writer) = tokio::io::split(client_io);

        // Run server loop in background task
        let server_task = tokio::spawn(async move {
            let buf_reader = tokio::io::BufReader::new(server_reader);
            let _ = server.run_stream_loop(buf_reader, server_writer).await;
        });

        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let mut client_lines = tokio::io::BufReader::new(client_reader).lines();

        // 1. Send initialize request
        let init_line = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n";
        client_writer.write_all(init_line.as_bytes()).await.unwrap();
        client_writer.flush().await.unwrap();

        let resp_line = client_lines.next_line().await.unwrap().expect("Response line");
        let resp: JsonRpcResponse = serde_json::from_str(&resp_line).expect("Parse response");
        assert_eq!(resp.id, Some(json!(1)));

        // 2. Send ping request
        let ping_line = "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\",\"params\":{}}\n";
        client_writer.write_all(ping_line.as_bytes()).await.unwrap();
        client_writer.flush().await.unwrap();

        let ping_resp_line = client_lines.next_line().await.unwrap().expect("Ping response line");
        let ping_resp: JsonRpcResponse = serde_json::from_str(&ping_resp_line).expect("Parse ping response");
        assert_eq!(ping_resp.id, Some(json!(2)));

        drop(client_lines);
        drop(client_writer);
        server_task.abort();
    }
}
