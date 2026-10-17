# OmniScope: Autonomous Dynamic Analysis & Introspection Sandbox

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![MCP Compliant](https://img.shields.io/badge/MCP-2024--11--05-brightgreen.svg)](https://modelcontextprotocol.io/)
[![Build & Tests](https://img.shields.io/badge/tests-38%2F38%20passing-success.svg)]()

**OmniScope** is an isolated dynamic analysis and introspection sandbox built from scratch in Rust, engineered specifically to grant autonomous AI agents:
1. **Physical & UI Agency:** Human-like biometric mouse movement (cubic Bézier curves with 5th-order smootherstep acceleration), natural keystroke timing jitter (60–150ms), native GDI BitBlt zero-copy screen capture, and accessibility tree parsing.
2. **Deep Runtime Observability:** Kernel-level observability across process lifecycles, filesystem mutations, registry persistence keys, and network activity with background noise suppression (<100 tokens per query).
3. **Temporal State Control:** Hardware-assisted Ring -1 hypervisor checkpointing (WHPX/KVM) and millisecond memory/disk rollbacks via QEMU QMP.
4. **Standardized AI Protocol:** Model Context Protocol (MCP) JSON-RPC 2.0 server operating over standard I/O (`stdio`).

---

## Architecture Overview

```
+--------------------------------------------------------------------------------+
|                             AI ASSISTANT / LLM                                 |
|            (Claude Desktop / Cursor IDE / Antigravity IDE / Agent)             |
+--------------------------------------------------------------------------------+
                                       |
                   JSON-RPC 2.0 over Stdio (Model Context Protocol)
                                       v
+--------------------------------------------------------------------------------+
|                         OMNISCOPE HOST CONTROL PLANE                           |
|  +---------------------------+  +-------------------------------------------+  |
|  |     MCP Server Engine     |  |       Hypervisor & Snapshot Manager       |  |
|  |  (10 Tools Declared)      |  |  - Ephemeral CoW Overlays (overlay.qcow2) |  |
|  +---------------------------+  |  - QMP Socket Driver (savevm / loadvm)   |  |
|               |                 |  - Anti-Analysis Cloaking (SMBIOS/CPUID)  |  |
|               v                 +-------------------------------------------+  |
|  +---------------------------+                       |                         |
|  |   Multiplexed IPC Client  |                       | QMP Port (4444)         |
|  |   (Seq ID / Tokio Duplex) |                       v                         |
+---------------+--------------------------------------+-------------------------+
                |                                      |
                | Virtio-Serial / Duplex               | Hardware-Assisted SLAT/EPT
                v                                      v
+--------------------------------------------------------------------------------+
|                       OMNISCOPE GUEST / VIRTUAL MACHINE                        |
|  +--------------------------------------------------------------------------+  |
|  |                           Guest Dispatcher                               |  |
|  +--------------------------------------------------------------------------+  |
|         |                                 |                        |           |
|         v                                 v                        v           |
|  +-----------------+           +--------------------+     +-----------------+  |
|  | Actuation Core  |           | Perception Core    |     | Telemetry Engine|  |
|  | - Bézier Mouse  |           | - GDI BitBlt       |     | - Process Tap   |  |
|  | - Keyboard Plan |           | - UI Auto (Tree)   |     | - Filesystem Tap|  |
|  | - Win32 Drivers |           | - Zero-Copy WebP   |     | - Registry Tap  |  |
|  +-----------------+           +--------------------+     | - Network Tap   |  |
|                                                           | - Noise Filter  |  |
|                                                           +-----------------+  |
+--------------------------------------------------------------------------------+
```

---

## Key Capabilities

### 1. Model Context Protocol (MCP) Tool Suite
OmniScope exposes 10 fine-grained tools over JSON-RPC 2.0 stdio:

| Tool Name | Parameters | Description |
|:---|:---|:---|
| `sandbox_status` | None | Reports hypervisor health, guest agent connectivity, and active checkpoints. |
| `sandbox_snapshot_create` | `snapshot_name` | Freezes RAM and records CoW disk delta in <300ms. |
| `sandbox_snapshot_revert` | `snapshot_name` | Instantly rolls back guest memory and filesystem state to a saved checkpoint. |
| `sandbox_launch_app` | `executable_path`, `arguments` | Launches target application and automatically binds telemetry taps to the process tree. |
| `sandbox_get_screen` | `crop_active_window`, `format` | Captures zero-copy desktop visual frame buffer (WebP / PNG / JPEG). |
| `sandbox_get_ui_tree` | `max_depth`, `window_filter` | Extracts accessibility tree with element bounds, control types, and interactive states. |
| `sandbox_click` | `x`, `y`, `button`, `click_type` | Injects natural human-like cubic Bézier curves and executes clicks. |
| `sandbox_type_text` | `text`, `press_enter` | Injects biometric keystrokes with variable timing jitter (60–150ms). |
| `sandbox_query_telemetry` | `source`, `limit`, `pid` | Returns a token-optimized semantic diff of file, registry, network, or process events. |
| `sandbox_inspect_memory` | `pid`, `query_type` | Scans memory for unpacked strings, loaded DLL modules, and open OS handles. |

### 2. Anti-Analysis & Virtualization Cloaking
OmniScope masks hardware and hypervisor artifacts using custom QEMU synthesis flags:
- **CPU:** `-cpu host,hypervisor=off,kvm=off`
- **SMBIOS:** Authentic Dell OptiPlex 7090 hardware tables (Type 1 manufacturer, product, and serial).
- **Storage:** Native NVMe serial numbers (`SAMSUNG-MZVLB1T0HBLR-0000`).
- **Network:** Authentic Dell OUI OEM MAC addresses (`00:1A:A0:5B:3F:82`).

### 3. Native Web App Testing & Automation
Eliminates reliance on external headless browser test runners (Playwright / Puppeteer) whose binary downloads frequently fail on restricted CDNs. OmniScope operates genuine browsers (Edge / Chrome) via native Windows GDI perception and biometric input drivers.

---

## Repository Structure

```
OmniScope-Sandbox/
├── .agents/
│   └── skills/
│       ├── omniscope/              # Autonomous reverse engineering & introspection playbook
│       └── omniscope-web-testing/  # Native browser & web app testing without Playwright
├── .cargo/
│   └── config.toml                 # LLVM-MinGW toolchain and static linking configuration
├── crates/
│   ├── omniscope-core/             # Binary framing (32-byte header, CRC-32), circular RingBuffer
│   ├── omniscope-guest/            # Actuation (Bézier), perception (GDI), and telemetry taps
│   └── omniscope-host/             # MCP server, QMP hypervisor manager, and host CLI
├── docs/
│   ├── 01_PLANNING_AND_FEASIBILITY.md
│   ├── 02_ARCHITECTURE_AND_DETAILED_DESIGN.md
│   ├── 03_DETAILED_CHUNKS_INTERACTIONS_AND_AI_PLAYBOOK.md
│   └── 04_OPERATIONAL_GUIDE_AND_MCP_INTEGRATION.md
├── Cargo.toml
└── README.md
```

---

## Building & Verification

### Prerequisites
- Rust `stable-x86_64-pc-windows-gnu` / `x86_64-pc-windows-gnullvm`
- LLVM-MinGW UCRT toolchain (Clang + LLVM-ar)

### Run Complete Test Suite
```powershell
cargo test --workspace
```
*Current test status: **38 passed, 0 failed** (includes full 10-step end-to-end AI Agent Playbook integration test).*

---

## MCP Setup Guide

### 1. Cursor IDE
Add to your `mcp.json`:
```json
{
  "mcpServers": {
    "omniscope": {
      "command": "target\\x86_64-pc-windows-gnullvm\\debug\\omniscope-host.exe",
      "args": ["--mcp", "--mock"]
    }
  }
}
```

### 2. Claude Desktop
Add to `%APPDATA%\Claude\claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "omniscope": {
      "command": "E:\\Sandbox_project\\target\\x86_64-pc-windows-gnullvm\\debug\\omniscope-host.exe",
      "args": ["--mcp", "--mock"]
    }
  }
}
```

### 3. Antigravity IDE
Add to `~/.gemini/config/mcp_config.json` or workspace `.agents/mcp_config.json`:
```json
{
  "mcpServers": {
    "omniscope": {
      "command": "E:\\Sandbox_project\\target\\x86_64-pc-windows-gnullvm\\debug\\omniscope-host.exe",
      "args": ["--mcp", "--mock"]
    }
  }
}
```

---

## License
Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.
