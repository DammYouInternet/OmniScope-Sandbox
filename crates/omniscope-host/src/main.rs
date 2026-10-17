use omniscope_guest::actuation::MockInputDriver;
use omniscope_guest::ipc::GuestDispatcher;
use omniscope_guest::perception::{MockFrameGrabber, MockUiTreeWalker};
use omniscope_guest::telemetry::TapManager;
use omniscope_host::ipc::HostIpcClient;
use omniscope_host::mcp::McpServer;
use omniscope_host::vm::{build_qemu_args, VmConfig};
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Direct all tracing logs to stderr to keep stdout strictly pristine for MCP JSON-RPC 2.0
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("OmniScope Host Control Plane & MCP Server");
        eprintln!("Usage: omniscope-host [OPTIONS]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --mcp               Run Model Context Protocol (MCP) JSON-RPC 2.0 server over stdio");
        eprintln!("  --connect <IP:PORT> Connect to active OmniScope guest agent (e.g. 127.0.0.1:5000)");
        eprintln!("  --mock              Use internal mock guest agent loopback (standalone mode)");
        eprintln!("  --qemu-args         Output generated QEMU stealth arguments and exit");
        eprintln!("  --help, -h          Display this help menu");
        return Ok(());
    }

    if args.iter().any(|a| a == "--qemu-args") {
        let config = VmConfig::default();
        let qemu_args = build_qemu_args(&config);
        println!("Generated QEMU Stealth Arguments:");
        for arg in qemu_args {
            print!("{} ", arg);
        }
        println!();
        return Ok(());
    }

    let connect_target = args.windows(2).find_map(|w| {
        if w[0] == "--connect" {
            Some(w[1].clone())
        } else {
            None
        }
    });

    let run_mock = args.iter().any(|a| a == "--mock") || connect_target.is_none();
    let run_mcp = args.iter().any(|a| a == "--mcp") || (!args.iter().any(|a| a == "--status"));

    // Set up HostIpcClient
    let client: HostIpcClient = if let Some(target) = connect_target {
        tracing::info!("Connecting to remote guest agent at {}...", target);
        let stream = TcpStream::connect(&target).await?;
        tracing::info!("Successfully connected to guest agent at {}", target);
        HostIpcClient::new(stream)
    } else if run_mock {
        tracing::info!("Starting in-process simulated guest agent loopback...");
        let (host_stream, guest_stream) = tokio::io::duplex(64 * 1024);

        let input_driver = Arc::new(MockInputDriver::new(1920, 1080));
        let frame_grabber = Arc::new(MockFrameGrabber::new(1920, 1080));
        let ui_walker = Arc::new(MockUiTreeWalker::new());
        let tap_manager = Arc::new(TapManager::default());

        let dispatcher = Arc::new(GuestDispatcher::new(
            input_driver,
            frame_grabber,
            ui_walker,
            tap_manager,
        ));

        tokio::spawn(async move {
            let _ = dispatcher.run_loop(guest_stream).await;
        });

        HostIpcClient::new(host_stream)
    } else {
        eprintln!("Error: specify either --connect <IP:PORT> or --mock");
        std::process::exit(1);
    };

    if run_mcp {
        tracing::info!("OmniScope MCP Server listening on stdio (JSON-RPC 2.0)...");
        let server = McpServer::new(client);
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        server.run_stream_loop(reader, stdout).await?;
        tracing::info!("MCP stdio session terminated cleanly.");
    }

    Ok(())
}
