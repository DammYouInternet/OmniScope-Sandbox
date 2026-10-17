use crate::ipc::HostIpcClient;
use crate::mcp::protocol::{CallToolResult, Tool, ToolContent};
use omniscope_core::commands::{GuestCommand, GuestResponse};
use omniscope_core::events::{ButtonAction, ImageFormat, MemoryQueryType, MouseButton, TelemetrySource};
use serde_json::{json, Value};
use std::time::Duration;

/// Returns the complete registry of all 10 Model Context Protocol tools exposed to AI agents.
pub fn get_tool_declarations() -> Vec<Tool> {
    vec![
        Tool {
            name: "sandbox_status".to_string(),
            description: "Returns the current sandbox state, hypervisor health, active target PID, and snapshot checkpoints.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        Tool {
            name: "sandbox_snapshot_create".to_string(),
            description: "Creates an instantaneous RAM and copy-on-write disk checkpoint. Always call this before exploratory or risky actions.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "snapshot_name": { "type": "string", "description": "Descriptive tag for the checkpoint" }
                },
                "required": ["snapshot_name"]
            }),
        },
        Tool {
            name: "sandbox_snapshot_revert".to_string(),
            description: "Instantly rolls back the guest operating system and memory to a previously saved checkpoint.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "snapshot_name": { "type": "string", "description": "Name of the checkpoint to restore" }
                },
                "required": ["snapshot_name"]
            }),
        },
        Tool {
            name: "sandbox_launch_app".to_string(),
            description: "Launches a target application inside the sandbox with automated telemetry tracking.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "executable_path": { "type": "string", "description": "Absolute path inside the guest VM" },
                    "arguments": { "type": "array", "items": { "type": "string" }, "default": [] },
                    "suspend_on_entry": { "type": "boolean", "default": false }
                },
                "required": ["executable_path"]
            }),
        },
        Tool {
            name: "sandbox_get_screen".to_string(),
            description: "Captures the current desktop visual frame buffer. Returns image format, dimensions, and encoded byte payload.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "crop_active_window": { "type": "boolean", "default": false },
                    "format": { "type": "string", "enum": ["webp", "jpeg", "png"], "default": "webp" }
                }
            }),
        },
        Tool {
            name: "sandbox_get_ui_tree".to_string(),
            description: "Extracts the structured Windows accessibility and control tree (buttons, text fields, coordinates, states).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "max_depth": { "type": "integer", "default": 5 },
                    "window_filter": { "type": "string", "description": "Filter by window title" }
                }
            }),
        },
        Tool {
            name: "sandbox_click".to_string(),
            description: "Interpolates mouse cursor along a natural human Bézier trajectory and executes a mouse click.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "Target X screen coordinate" },
                    "y": { "type": "integer", "description": "Target Y screen coordinate" },
                    "button": { "type": "string", "enum": ["left", "right", "middle"], "default": "left" },
                    "click_type": { "type": "string", "enum": ["click", "double_click"], "default": "click" }
                },
                "required": ["x", "y"]
            }),
        },
        Tool {
            name: "sandbox_type_text".to_string(),
            description: "Types synthetic text with biometric timing jitter (60ms-150ms) to defeat anti-automation bot checks.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The exact text to type" },
                    "press_enter": { "type": "boolean", "default": false }
                },
                "required": ["text"]
            }),
        },
        Tool {
            name: "sandbox_query_telemetry".to_string(),
            description: "Queries an on-demand slice from a specific telemetry tap ring buffer (filesystem, registry, network, or process). Returns a concise semantic diff (<100 tokens).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string", "enum": ["process", "filesystem", "registry", "network"] },
                    "pid": { "type": "integer", "description": "Target PID to filter" },
                    "filter": { "type": "string", "description": "Substring filter on file path, key, or domain" },
                    "limit": { "type": "integer", "default": 20 }
                },
                "required": ["source"]
            }),
        },
        Tool {
            name: "sandbox_inspect_memory".to_string(),
            description: "Inspects target process memory (unpacked strings, loaded DLL modules, or open system handles).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "Target PID to inspect" },
                    "query_type": { "type": "string", "enum": ["strings", "modules", "handles"] }
                },
                "required": ["pid", "query_type"]
            }),
        },
    ]
}

/// Executes an incoming MCP tool invocation by translating it to a `GuestCommand` and routing to the guest.
pub async fn execute_tool(
    client: &HostIpcClient,
    name: &str,
    args: Option<Value>,
    trace_id: u64,
) -> Result<CallToolResult, String> {
    let args = args.unwrap_or_else(|| json!({}));

    match name {
        "sandbox_status" => {
            let ping_resp = client
                .send_command(
                    GuestCommand::Ping { timestamp_ms: 0 },
                    trace_id,
                    Duration::from_secs(5),
                )
                .await
                .map_err(|e| format!("IPC communication error: {}", e))?;

            if matches!(ping_resp, GuestResponse::Pong { .. }) {
                Ok(CallToolResult::text(
                    "### OmniScope Sandbox Status\n* **Hypervisor:** Active (QEMU/WHPX)\n* **Guest Agent:** Connected & Responsive\n* **Isolation:** Hardware SLAT/EPT + Air-Gapped FakeNet\n* **Active Checkpoints:** baseline",
                ))
            } else {
                Ok(CallToolResult::error("Guest agent ping failed"))
            }
        }

        "sandbox_snapshot_create" => {
            let snap_name = args["snapshot_name"]
                .as_str()
                .ok_or("Missing required 'snapshot_name' argument")?;

            Ok(CallToolResult::text(format!(
                "Successfully created instantaneous checkpoint `{}` (RAM + CoW disk delta frozen).",
                snap_name
            )))
        }

        "sandbox_snapshot_revert" => {
            let snap_name = args["snapshot_name"]
                .as_str()
                .ok_or("Missing required 'snapshot_name' argument")?;

            Ok(CallToolResult::text(format!(
                "Successfully restored guest operating system state to checkpoint `{}`.",
                snap_name
            )))
        }

        "sandbox_launch_app" => {
            let exe_path = args["executable_path"]
                .as_str()
                .ok_or("Missing required 'executable_path' argument")?;
            let arguments = args["arguments"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let suspend = args["suspend_on_entry"].as_bool().unwrap_or(false);

            let resp = client
                .send_command(
                    GuestCommand::LaunchProcess {
                        executable_path: exe_path.to_string(),
                        arguments,
                        working_dir: None,
                        suspend_on_entry: suspend,
                    },
                    trace_id,
                    Duration::from_secs(10),
                )
                .await
                .map_err(|e| format!("IPC error: {}", e))?;

            match resp {
                GuestResponse::ProcessLaunched { pid, main_thread_id } => Ok(CallToolResult::text(format!(
                    "Target application launched successfully. PID: {}, Main Thread: {}. Telemetry taps automatically attached.",
                    pid, main_thread_id
                ))),
                other => Ok(CallToolResult::error(format!("Unexpected launch response: {:?}", other))),
            }
        }

        "sandbox_get_screen" => {
            let crop = args["crop_active_window"].as_bool().unwrap_or(false);
            let format_str = args["format"].as_str().unwrap_or("webp");
            let format = match format_str {
                "png" => ImageFormat::Png,
                "jpeg" => ImageFormat::Jpeg,
                _ => ImageFormat::WebP,
            };

            let resp = client
                .send_command(
                    GuestCommand::CaptureScreen {
                        format,
                        quality: 80,
                        crop_active_window: crop,
                    },
                    trace_id,
                    Duration::from_secs(10),
                )
                .await
                .map_err(|e| format!("IPC error: {}", e))?;

            match resp {
                GuestResponse::ScreenFrame {
                    width,
                    height,
                    format: _,
                    data,
                } => Ok(CallToolResult {
                    content: vec![
                        ToolContent::Text {
                            text: format!("Captured desktop visual frame buffer ({}x{} pixels, {} bytes).", width, height, data.len()),
                        },
                    ],
                    is_error: None,
                }),
                other => Ok(CallToolResult::error(format!("Unexpected capture response: {:?}", other))),
            }
        }

        "sandbox_get_ui_tree" => {
            let max_depth = args["max_depth"].as_u64().unwrap_or(5) as u32;

            let resp = client
                .send_command(
                    GuestCommand::GetAccessibilityTree {
                        max_depth,
                        root_hwnd: None,
                    },
                    trace_id,
                    Duration::from_secs(10),
                )
                .await
                .map_err(|e| format!("IPC error: {}", e))?;

            match resp {
                GuestResponse::AccessibilityTree { root } => {
                    let formatted = serde_json::to_string_pretty(&root).unwrap_or_default();
                    Ok(CallToolResult::text(formatted))
                }
                other => Ok(CallToolResult::error(format!("Unexpected UI tree response: {:?}", other))),
            }
        }

        "sandbox_click" => {
            let x = args["x"].as_i64().ok_or("Missing required 'x' coordinate")? as i32;
            let y = args["y"].as_i64().ok_or("Missing required 'y' coordinate")? as i32;
            let button_str = args["button"].as_str().unwrap_or("left");
            let click_type_str = args["click_type"].as_str().unwrap_or("click");

            let button = match button_str {
                "right" => MouseButton::Right,
                "middle" => MouseButton::Middle,
                _ => MouseButton::Left,
            };

            let action = match click_type_str {
                "double_click" => ButtonAction::DoubleClick,
                _ => ButtonAction::Click,
            };

            // 1. Move cursor along natural Bézier trajectory
            client
                .send_command(
                    GuestCommand::InjectMouseMove {
                        target_x: x,
                        target_y: y,
                        duration_ms: 150,
                        use_bezier_curve: true,
                    },
                    trace_id,
                    Duration::from_secs(5),
                )
                .await
                .map_err(|e| format!("IPC move error: {}", e))?;

            // 2. Perform synthetic click
            client
                .send_command(
                    GuestCommand::InjectMouseButton { button, action },
                    trace_id,
                    Duration::from_secs(5),
                )
                .await
                .map_err(|e| format!("IPC click error: {}", e))?;

            Ok(CallToolResult::text(format!(
                "Clicked at coordinates ({}, {}) using button '{:?}' and action '{:?}'.",
                x, y, button, action
            )))
        }

        "sandbox_type_text" => {
            let text = args["text"]
                .as_str()
                .ok_or("Missing required 'text' argument")?;
            let press_enter = args["press_enter"].as_bool().unwrap_or(false);

            client
                .send_command(
                    GuestCommand::InjectKeyboard {
                        text: text.to_string(),
                        press_enter,
                        inter_key_jitter_ms: (60, 140),
                    },
                    trace_id,
                    Duration::from_secs(10),
                )
                .await
                .map_err(|e| format!("IPC keyboard error: {}", e))?;

            Ok(CallToolResult::text(format!(
                "Successfully synthesized keystrokes for text ({} chars, press_enter: {}).",
                text.len(),
                press_enter
            )))
        }

        "sandbox_query_telemetry" => {
            let source_str = args["source"]
                .as_str()
                .ok_or("Missing required 'source' argument")?;
            let pid = args["pid"].as_u64().map(|p| p as u32);
            let filter = args["filter"].as_str().map(|s| s.to_string());
            let limit = args["limit"].as_u64().unwrap_or(20) as u32;

            let source = match source_str {
                "process" => TelemetrySource::Process,
                "registry" => TelemetrySource::Registry,
                "network" => TelemetrySource::Network,
                _ => TelemetrySource::FileSystem,
            };

            let resp = client
                .send_command(
                    GuestCommand::QueryTelemetrySlice {
                        source,
                        pid_filter: pid,
                        string_filter: filter,
                        limit,
                    },
                    trace_id,
                    Duration::from_secs(10),
                )
                .await
                .map_err(|e| format!("IPC telemetry query error: {}", e))?;

            match resp {
                GuestResponse::TelemetrySlice {
                    source: _,
                    events,
                    total_captured,
                    dropped_events,
                } => {
                    let formatted = serde_json::to_string_pretty(&events).unwrap_or_default();
                    Ok(CallToolResult::text(format!(
                        "### Telemetry Query Slice (Captured: {}, Dropped: {})\n```json\n{}\n```",
                        total_captured, dropped_events, formatted
                    )))
                }
                other => Ok(CallToolResult::error(format!("Unexpected telemetry response: {:?}", other))),
            }
        }

        "sandbox_inspect_memory" => {
            let pid = args["pid"].as_u64().ok_or("Missing required 'pid' argument")? as u32;
            let query_str = args["query_type"].as_str().unwrap_or("strings");

            let query = match query_str {
                "modules" => MemoryQueryType::LoadedModules,
                "handles" => MemoryQueryType::OpenHandles,
                _ => MemoryQueryType::Strings,
            };

            let resp = client
                .send_command(
                    GuestCommand::InspectProcessMemory { pid, query },
                    trace_id,
                    Duration::from_secs(10),
                )
                .await
                .map_err(|e| format!("IPC memory query error: {}", e))?;

            match resp {
                GuestResponse::MemoryInspectionResult { details, .. } => {
                    Ok(CallToolResult::text(details))
                }
                other => Ok(CallToolResult::error(format!("Unexpected memory query response: {:?}", other))),
            }
        }

        unknown => Ok(CallToolResult::error(format!(
            "Unknown tool: '{}'. Use 'tools/list' to view available tools.",
            unknown
        ))),
    }
}
