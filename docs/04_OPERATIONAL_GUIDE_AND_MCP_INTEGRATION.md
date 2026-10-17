# OmniScope Operational Guide & MCP Client Integration

## 1. Overview
OmniScope is a high-performance, isolated dynamic analysis and introspection sandbox purpose-built to give AI agents:
1. **Physical/UI Agency:** Biometric cubic Bézier mouse paths, variable keystroke jitter, and accessibility tree parsing.
2. **Deep Runtime Observability:** Process, filesystem, registry, and network kernel taps with background noise suppression.
3. **Temporal State Control:** Hardware-assisted Ring -1 snapshot checkpointing and millisecond rollbacks via QEMU QMP.
4. **Standardized AI Protocol:** Model Context Protocol (MCP) JSON-RPC 2.0 server operating over standard I/O (`stdio`).

---

## 2. Quickstart & Modes of Operation

### Mode A: Standalone Mock / Evaluation Mode (Zero Setup)
OmniScope Host includes an internal loopback guest agent and mock driver pipeline. This allows AI assistants (Claude, Cursor, Antigravity IDE) to connect and run the complete introspection playbook immediately without needing a running QEMU Windows VM.

```powershell
# Run MCP server on stdio in mock mode
omniscope-host.exe --mcp --mock
```

### Mode B: Full Hardware-Isolated QEMU Virtual Machine
In production, OmniScope coordinates hardware virtualization cloaking and connects to an in-guest daemon.

#### Step 1: Launch QEMU with Anti-Analysis Cloaking
```powershell
# Inspect the generated stealth arguments
omniscope-host.exe --qemu-args

# Launch QEMU with disposable CoW overlay
qemu-system-x86_64 `
  -machine q35 `
  -accel whpx,kernel-irqchip=off `
  -m 8192M -smp cpus=4 `
  -cpu host,hypervisor=off,kvm=off `
  -smbios type=1,manufacturer="Dell Inc.",product="OptiPlex 7090",serial="7X89B22" `
  -drive file=images/overlay.qcow2,if=none,id=system_drive,format=qcow2 `
  -device nvme,serial=SAMSUNG-MZVLB1T0HBLR-0000,drive=system_drive `
  -netdev user,id=net0 `
  -device e1000,netdev=net0,mac=00:1A:A0:5B:3F:82 `
  -qmp tcp:127.0.0.1:4444,server,nowait `
  -device virtio-serial `
  -chardev socket,id=omniscope_ch,host=127.0.0.1,port=5000,server,nowait `
  -device virtserialport,chardev=omniscope_ch,name=omniscope.guest.0 `
  -display none
```

#### Step 2: Start In-Guest Daemon (Inside Windows VM)
```powershell
omniscope-guest.exe --port 5000
```

#### Step 3: Start Host MCP Server
```powershell
omniscope-host.exe --mcp --connect 127.0.0.1:5000
```

---

## 3. Integrating with AI Environments (MCP Setup)

### Cursor IDE Configuration
Add OmniScope to your `~/.cursor/mcp.json` or workspace `.cursor/mcp.json`:
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

### Claude Desktop Configuration
Add OmniScope to `%APPDATA%\\Claude\\claude_desktop_config.json`:
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

### Antigravity IDE Configuration
Add OmniScope to `.agents/mcp_config.json`:
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

## 4. Complete AI Agent Tool Catalog

OmniScope provides 10 fine-grained MCP tools:

| Tool Name | Parameters | Purpose |
|:---|:---|:---|
| `sandbox_status` | None | Queries hypervisor isolation, guest agent connectivity, and active checkpoints. |
| `sandbox_snapshot_create` | `snapshot_name` (string) | Freezes RAM and creates CoW disk delta snapshot in <300ms. |
| `sandbox_snapshot_revert` | `snapshot_name` (string) | Reverts VM to specified snapshot state. |
| `sandbox_launch_app` | `executable_path` (string), `arguments` (array), `suspend_on_entry` (bool) | Launches target process with automatic child PID tracking and telemetry tap adoption. |
| `sandbox_get_screen` | `crop_active_window` (bool), `format` (webp/png/jpeg) | Captures visual frame buffer using zero-copy GDI/BitBlt. |
| `sandbox_get_ui_tree` | `max_depth` (int), `filter` (string) | Extracts structured UI accessibility tree with element bounds, IDs, and control types. |
| `sandbox_click` | `x` (int), `y` (int), `button` (left/right/middle), `double_click` (bool) | Injects human-like mouse movement via cubic Bézier curves and sends click event. |
| `sandbox_type_text` | `text` (string), `press_enter` (bool) | Injects biometric keystrokes with 60–150ms Gaussian timing jitter. |
| `sandbox_query_telemetry` | `source` (process/filesystem/registry/network), `limit` (int), `pid_filter` (int) | Queries token-optimized telemetry slice with background OS noise suppressed. |
| `sandbox_inspect_memory` | `pid` (int), `query_type` (strings/modules/handles) | Scans process memory space for indicators of compromise. |

---

## 5. Verification Test Suite Results

The codebase has completed 100% of Chunk verification and End-to-End integration testing:

```
running 10 tests in omniscope_core (binary framing, CRC-32, ring buffer) ... ok
running 13 tests in omniscope_guest (Bézier mouse, typing jitter, GDI grabber, UIA walker, taps, noise filter) ... ok
running 14 tests in omniscope_host (IPC client, QMP socket, stealth args, VmManager, MCP server) ... ok
running 1 test in e2e_ai_playbook (Full 10-step AI Agent Playbook) ... ok

Total: 38 passed; 0 failed; 0 ignored; finished in 0.35s
```

---

## 6. Native Web Application Testing (Without Playwright Dependencies)

When verifying web applications, AI assistants can use OmniScope's native Win32/GDI stack instead of downloading external Playwright/Chromium driver binaries (`playwright-*-win32_x64.zip`):

1. **Pre-test snapshot:** Call `sandbox_snapshot_create(snapshot_name="web_test_clean")`.
2. **Launch sandbox browser:** Call `sandbox_launch_app` with `msedge.exe` or `chrome.exe` pointing to `http://localhost:3000`.
3. **Capture visual rendering:** Call `sandbox_get_screen` for instant zero-copy frame buffer verification.
4. **Locate DOM/UI elements:** Call `sandbox_get_ui_tree` to extract element coordinates.
5. **Actuate inputs:** Call `sandbox_click` and `sandbox_type_text` to test forms, buttons, and navigation.
6. **Inspect backend network traffic:** Call `sandbox_query_telemetry(source="network")` to verify API calls and DNS queries.
7. **Clean rollback:** Call `sandbox_snapshot_revert(snapshot_name="web_test_clean")` to instantly wipe cookies, storage, and state.

Refer to the Antigravity skill at [`.agents/skills/omniscope-web-testing/SKILL.md`](file:///e:/Sandbox_project/.agents/skills/omniscope-web-testing/SKILL.md) for automated execution.

