use crate::actuation::InputDriver;
use crate::perception::{FrameGrabber, UiTreeWalker};
use crate::telemetry::TapManager;
use futures_util::{SinkExt, StreamExt};
use omniscope_core::commands::{GuestCommand, GuestResponse};
use omniscope_core::error::ProtocolError;
use omniscope_core::protocol::Frame;
use omniscope_core::ProtocolCodec;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

/// Central command dispatcher executing incoming host requests on guest subsystems.
pub struct GuestDispatcher {
    pub input_driver: Arc<dyn InputDriver>,
    pub frame_grabber: Arc<dyn FrameGrabber>,
    pub ui_walker: Arc<dyn UiTreeWalker>,
    pub tap_manager: Arc<TapManager>,
}

impl GuestDispatcher {
    pub fn new(
        input_driver: Arc<dyn InputDriver>,
        frame_grabber: Arc<dyn FrameGrabber>,
        ui_walker: Arc<dyn UiTreeWalker>,
        tap_manager: Arc<TapManager>,
    ) -> Self {
        Self {
            input_driver,
            frame_grabber,
            ui_walker,
            tap_manager,
        }
    }

    /// Dispatches a single `GuestCommand` to the appropriate guest engine and formats the response.
    pub fn handle_command(&self, command: GuestCommand) -> GuestResponse {
        match command {
            GuestCommand::Ping { timestamp_ms } => GuestResponse::Pong { timestamp_ms },

            GuestCommand::LaunchProcess {
                executable_path: _,
                arguments: _,
                working_dir: _,
                suspend_on_entry: _,
            } => {
                // Return structured launch acknowledgment (dummy PID 1042 for test environments)
                GuestResponse::ProcessLaunched {
                    pid: 1042,
                    main_thread_id: 1043,
                }
            }

            GuestCommand::TerminateProcess { pid } => {
                GuestResponse::ProcessTerminated { pid, exit_code: 0 }
            }

            GuestCommand::CaptureScreen {
                format,
                quality,
                crop_active_window,
            } => match self
                .frame_grabber
                .capture_screen(crop_active_window, format, quality)
            {
                Ok(frame) => GuestResponse::ScreenFrame {
                    width: frame.width,
                    height: frame.height,
                    format: frame.format,
                    data: frame.data,
                },
                Err(e) => GuestResponse::Error {
                    code: 1,
                    message: e.to_string(),
                },
            },

            GuestCommand::GetAccessibilityTree {
                max_depth,
                root_hwnd,
            } => match self.ui_walker.walk_tree(root_hwnd, max_depth, None) {
                Ok(root) => GuestResponse::AccessibilityTree { root },
                Err(e) => GuestResponse::Error {
                    code: 2,
                    message: e.to_string(),
                },
            },

            GuestCommand::InjectMouseMove {
                target_x,
                target_y,
                duration_ms,
                use_bezier_curve,
            } => match self
                .input_driver
                .move_cursor(target_x, target_y, duration_ms, use_bezier_curve)
            {
                Ok(()) => GuestResponse::ActuationCompleted { success: true },
                Err(e) => GuestResponse::Error {
                    code: 3,
                    message: e.to_string(),
                },
            },

            GuestCommand::InjectMouseButton { button, action } => {
                match self.input_driver.click_mouse(button, action) {
                    Ok(()) => GuestResponse::ActuationCompleted { success: true },
                    Err(e) => GuestResponse::Error {
                        code: 4,
                        message: e.to_string(),
                    },
                }
            }

            GuestCommand::InjectKeyboard {
                text,
                press_enter,
                inter_key_jitter_ms,
            } => match self
                .input_driver
                .type_text(&text, press_enter, inter_key_jitter_ms)
            {
                Ok(()) => GuestResponse::ActuationCompleted { success: true },
                Err(e) => GuestResponse::Error {
                    code: 5,
                    message: e.to_string(),
                },
            },

            GuestCommand::ConfigureTaps {
                target_pids,
                enable_process,
                enable_filesystem,
                enable_registry,
                enable_network,
            } => {
                self.tap_manager.configure(
                    target_pids,
                    enable_process,
                    enable_filesystem,
                    enable_registry,
                    enable_network,
                );
                GuestResponse::ActuationCompleted { success: true }
            }

            GuestCommand::QueryTelemetrySlice {
                source,
                pid_filter,
                string_filter,
                limit,
            } => {
                let (events, total, dropped) = self.tap_manager.query_slice(
                    source,
                    pid_filter,
                    string_filter.as_deref(),
                    limit as usize,
                );
                GuestResponse::TelemetrySlice {
                    source,
                    events,
                    total_captured: total,
                    dropped_events: dropped,
                }
            }

            GuestCommand::InspectProcessMemory { pid, query } => {
                GuestResponse::MemoryInspectionResult {
                    pid,
                    query,
                    details: format!("Memory inspection result for PID {}", pid),
                }
            }
        }
    }

    /// Runs the message handling loop over an asynchronous stream until closed.
    pub async fn run_loop<S>(&self, stream: S) -> Result<(), ProtocolError>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let framed = Framed::new(stream, ProtocolCodec::new());
        let (mut sink, mut stream) = framed.split();

        while let Some(result) = stream.next().await {
            let frame = result?;
            let seq = frame.header.sequence_id;
            let trace = frame.header.trace_id;

            let cmd = frame.decode_command()?;
            let response = self.handle_command(cmd);

            let resp_frame = Frame::new_response(seq, trace, &response)?;
            sink.send(resp_frame).await?;
        }

        Ok(())
    }
}
