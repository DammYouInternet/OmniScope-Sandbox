use omniscope_guest::actuation::{InputDriver, MockInputDriver};
use omniscope_guest::ipc::GuestDispatcher;
use omniscope_guest::perception::{FrameGrabber, MockFrameGrabber, MockUiTreeWalker, UiTreeWalker};
use omniscope_guest::telemetry::TapManager;
use std::sync::Arc;
use tokio::net::TcpListener;

#[cfg(windows)]
use omniscope_guest::actuation::WindowsInputDriver;
#[cfg(windows)]
use omniscope_guest::perception::{WindowsScreenGrabber, WindowsUiTreeWalker};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("OmniScope Guest Daemon");
        println!("Usage: omniscope-guest [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --port <PORT>   Port to listen on (default: 5000)");
        println!("  --host <IP>     Host IP address to bind (default: 127.0.0.1)");
        println!("  --mock          Force mock input/screen/UI drivers (headless/testing)");
        println!("  --help, -h      Display this help menu");
        return Ok(());
    }

    let mut port = 5000u16;
    let mut host = "127.0.0.1".to_string();
    let mut use_mock = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(5000);
                    i += 1;
                }
            }
            "--host" => {
                if i + 1 < args.len() {
                    host = args[i + 1].clone();
                    i += 1;
                }
            }
            "--mock" => {
                use_mock = true;
            }
            _ => {}
        }
        i += 1;
    }

    tracing::info!(
        "Initializing OmniScope Guest Agent [PID: {}]",
        std::process::id()
    );

    // 2. Instantiate drivers based on operating mode
    #[cfg(windows)]
    let (input_driver, frame_grabber, ui_walker): (
        Arc<dyn InputDriver>,
        Arc<dyn FrameGrabber>,
        Arc<dyn UiTreeWalker>,
    ) = if use_mock {
        tracing::info!("Running with Mock Perception & Actuation drivers");
        (
            Arc::new(MockInputDriver::new(1920, 1080)),
            Arc::new(MockFrameGrabber::new(1920, 1080)),
            Arc::new(MockUiTreeWalker::new()),
        )
    } else {
        tracing::info!("Initializing Native Win32 Subsystems (GDI BitBlt + User32 SendInput + UIA Walker)");
        (
            Arc::new(WindowsInputDriver::new()),
            Arc::new(WindowsScreenGrabber::new()),
            Arc::new(WindowsUiTreeWalker::new()),
        )
    };

    #[cfg(not(windows))]
    let (input_driver, frame_grabber, ui_walker): (
        Arc<dyn InputDriver>,
        Arc<dyn FrameGrabber>,
        Arc<dyn UiTreeWalker>,
    ) = {
        tracing::info!("Non-Windows host detected; defaulting to Mock drivers");
        (
            Arc::new(MockInputDriver::new(1920, 1080)),
            Arc::new(MockFrameGrabber::new(1920, 1080)),
            Arc::new(MockUiTreeWalker::new()),
        )
    };

    let tap_manager = Arc::new(TapManager::default());
    let dispatcher = Arc::new(GuestDispatcher::new(
        input_driver,
        frame_grabber,
        ui_walker,
        tap_manager,
    ));

    // 3. Bind TCP listener
    let bind_addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&bind_addr).await?;
    tracing::info!("OmniScope Guest Agent listening on {}", bind_addr);

    // 4. Accept loop
    loop {
        tokio::select! {
            accept_res = listener.accept() => {
                match accept_res {
                    Ok((stream, peer_addr)) => {
                        tracing::info!("Host connection established from {}", peer_addr);
                        let disp = dispatcher.clone();
                        tokio::spawn(async move {
                            if let Err(e) = disp.run_loop(stream).await {
                                tracing::warn!("Session ended with error: {}", e);
                            } else {
                                tracing::info!("Session closed cleanly for {}", peer_addr);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("TCP accept error: {}", e);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, terminating guest agent...");
                break;
            }
        }
    }

    Ok(())
}
