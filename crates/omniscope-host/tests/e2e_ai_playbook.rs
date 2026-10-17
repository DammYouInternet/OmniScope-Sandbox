use omniscope_core::events::{
    FileOp, FileSystemEvent, ProcessEvent, ProcessEventType, TelemetryEvent,
};
use omniscope_guest::actuation::MockInputDriver;
use omniscope_guest::ipc::GuestDispatcher;
use omniscope_guest::perception::{MockFrameGrabber, MockUiTreeWalker};
use omniscope_guest::telemetry::TapManager;
use omniscope_host::ipc::HostIpcClient;
use omniscope_host::mcp::{JsonRpcRequest, JsonRpcResponse, McpServer};
use serde_json::json;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// End-to-End AI Agent Playbook Integration Test
/// Simulates an autonomous LLM agent (like Claude / GPT-4 / Gemini) interacting
/// through Model Context Protocol (MCP) JSON-RPC 2.0 stdio over duplex streams
/// to introspect, actuate, monitor, and rollback a sandboxed application.
#[tokio::test]
async fn test_end_to_end_ai_playbook_workflow() {
    // 1. Establish Duplex IPC Communication Pipe
    let (host_stream, guest_stream) = tokio::io::duplex(128 * 1024);

    // 2. Initialize In-Guest Instrumentation Subsystems
    let input_driver = Arc::new(MockInputDriver::new(1920, 1080));
    let frame_grabber = Arc::new(MockFrameGrabber::new(1280, 720));
    let ui_walker = Arc::new(MockUiTreeWalker::new());
    let tap_manager = Arc::new(TapManager::default());

    // Populate guest tap manager with sample process & filesystem activity
    // including background noise (svchost.exe) and target activity (target_app.exe)
    tap_manager.record_event(TelemetryEvent::Process(ProcessEvent {
        timestamp_ns: 1000,
        event_type: ProcessEventType::Start,
        pid: 9999,
        parent_pid: Some(4),
        image_name: "C:\\Windows\\System32\\svchost.exe".to_string(),
        command_line: Some("svchost.exe -k netsvcs".to_string()),
        dll_path: None,
    }));

    // Target process launch
    tap_manager.record_event(TelemetryEvent::Process(ProcessEvent {
        timestamp_ns: 2000,
        event_type: ProcessEventType::Start,
        pid: 4040,
        parent_pid: Some(1000),
        image_name: "C:\\Target\\MaliciousSample.exe".to_string(),
        command_line: Some("MaliciousSample.exe --stealth".to_string()),
        dll_path: None,
    }));

    // Target filesystem modification
    tap_manager.record_event(TelemetryEvent::FileSystem(FileSystemEvent {
        timestamp_ns: 3000,
        pid: 4040,
        operation: FileOp::Write,
        file_path: "C:\\Users\\Public\\ransom_note.txt".to_string(),
        bytes_transferred: Some(1024),
    }));

    let dispatcher = Arc::new(GuestDispatcher::new(
        input_driver.clone(),
        frame_grabber.clone(),
        ui_walker.clone(),
        tap_manager.clone(),
    ));

    // Spawn guest dispatch loop in background
    let guest_task = tokio::spawn(async move {
        let _ = dispatcher.run_loop(guest_stream).await;
    });

    // 3. Initialize Host MCP Server over HostIpcClient
    let host_client = HostIpcClient::new(host_stream);
    let mcp_server = McpServer::new(host_client);

    // Create virtual stdio duplex pipe simulating AI agent communicating with MCP server
    let (ai_io, server_io) = tokio::io::duplex(64 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_io);
    let (ai_reader, mut ai_writer) = tokio::io::split(ai_io);

    let server_task = tokio::spawn(async move {
        let buf_reader = BufReader::new(server_reader);
        let _ = mcp_server.run_stream_loop(buf_reader, server_writer).await;
    });

    let mut ai_lines = BufReader::new(ai_reader).lines();

    // Helper macro to send a JSON-RPC request and await response
    macro_rules! send_rpc {
        ($req:expr) => {{
            let mut line = serde_json::to_string(&$req).unwrap();
            line.push('\n');
            ai_writer.write_all(line.as_bytes()).await.unwrap();
            ai_writer.flush().await.unwrap();

            let resp_line = ai_lines.next_line().await.unwrap().expect("Response line");
            let resp: JsonRpcResponse = serde_json::from_str(&resp_line).expect("Parse response");
            resp
        }};
    }

    // -----------------------------------------------------------------------
    // STEP 1: AI MCP Handshake (`initialize`)
    // -----------------------------------------------------------------------
    let init_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: None,
    };
    let init_resp = send_rpc!(init_req);
    assert_eq!(init_resp.id, Some(json!(1)));
    let init_res = init_resp.result.unwrap();
    assert_eq!(init_res["protocolVersion"], "2024-11-05");
    assert_eq!(init_res["serverInfo"]["name"], "omniscope");

    // -----------------------------------------------------------------------
    // STEP 2: AI Tool Discovery (`tools/list`)
    // -----------------------------------------------------------------------
    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "tools/list".to_string(),
        params: None,
    };
    let list_resp = send_rpc!(list_req);
    let list_res = list_resp.result.unwrap();
    let tools = list_res["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 10, "Must expose all 10 MCP tools to the agent");

    // -----------------------------------------------------------------------
    // STEP 3: Check Sandbox Status (`sandbox_status`)
    // -----------------------------------------------------------------------
    let status_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_status",
            "arguments": {}
        })),
    };
    let status_resp = send_rpc!(status_req);
    let status_text = status_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(status_text.contains("OmniScope Sandbox Status"));
    assert!(status_text.contains("Connected & Responsive"));

    // -----------------------------------------------------------------------
    // STEP 4: Capture Golden Baseline Checkpoint (`sandbox_snapshot_create`)
    // -----------------------------------------------------------------------
    let snap_create_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(4)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_snapshot_create",
            "arguments": {
                "snapshot_name": "golden_pre_detonation"
            }
        })),
    };
    let snap_create_resp = send_rpc!(snap_create_req);
    let snap_create_text = snap_create_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(snap_create_text.contains("Successfully created instantaneous checkpoint `golden_pre_detonation`"));

    // -----------------------------------------------------------------------
    // STEP 5: Launch Target Binary in Sandbox (`sandbox_launch_app`)
    // -----------------------------------------------------------------------
    let launch_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(5)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_launch_app",
            "arguments": {
                "executable_path": "C:\\Target\\MaliciousSample.exe"
            }
        })),
    };
    let launch_resp = send_rpc!(launch_req);
    let launch_text = launch_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(launch_text.contains("Target application launched successfully"));

    // -----------------------------------------------------------------------
    // STEP 6: Zero-Copy Screen Perception (`sandbox_get_screen`)
    // -----------------------------------------------------------------------
    let screen_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(6)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_get_screen",
            "arguments": {
                "format": "webp"
            }
        })),
    };
    let screen_resp = send_rpc!(screen_req);
    let screen_text = screen_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(screen_text.contains("Captured desktop visual frame buffer (1280x720 pixels"));

    // -----------------------------------------------------------------------
    // STEP 7: Inspect UI Accessibility Tree (`sandbox_get_ui_tree`)
    // -----------------------------------------------------------------------
    let ui_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(7)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_get_ui_tree",
            "arguments": {
                "max_depth": 3
            }
        })),
    };
    let ui_resp = send_rpc!(ui_req);
    let ui_text = ui_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(ui_text.contains("Desktop"));
    assert!(ui_text.contains("Connect"));

    // -----------------------------------------------------------------------
    // STEP 8: Biometric Actuation (Click & Type)
    // -----------------------------------------------------------------------
    let click_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(8)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_click",
            "arguments": {
                "x": 640,
                "y": 360,
                "button": "left"
            }
        })),
    };
    let click_resp = send_rpc!(click_req);
    assert!(click_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Clicked at coordinates (640, 360)"));

    let type_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(9)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_type_text",
            "arguments": {
                "text": "AdminPassword123!",
                "press_enter": true
            }
        })),
    };
    let type_resp = send_rpc!(type_req);
    assert!(type_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Successfully synthesized keystrokes"));

    // -----------------------------------------------------------------------
    // STEP 9: Telemetry Harvest & Noise Suppression (`sandbox_query_telemetry`)
    // -----------------------------------------------------------------------
    let telem_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(10)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_query_telemetry",
            "arguments": {
                "source": "filesystem",
                "limit": 10
            }
        })),
    };
    let telem_resp = send_rpc!(telem_req);
    let telem_text = telem_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(telem_text.contains("Telemetry Query Slice"));
    assert!(telem_text.contains("ransom_note.txt"));

    // -----------------------------------------------------------------------
    // STEP 10: Revert to Clean Checkpoint (`sandbox_snapshot_revert`)
    // -----------------------------------------------------------------------
    let revert_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(11)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "sandbox_snapshot_revert",
            "arguments": {
                "snapshot_name": "golden_pre_detonation"
            }
        })),
    };
    let revert_resp = send_rpc!(revert_req);
    let revert_text = revert_resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(revert_text.contains("Successfully restored guest operating system state to checkpoint `golden_pre_detonation`"));

    // Clean teardown
    drop(ai_lines);
    drop(ai_writer);
    server_task.abort();
    guest_task.abort();
}
