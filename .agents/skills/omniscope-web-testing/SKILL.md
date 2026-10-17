---
name: omniscope-web-testing
description: Autonomous web application verification, UI testing, and network telemetry observation inside the OmniScope sandbox without external Playwright/Chromium driver downloads.
---

# OmniScope Native Web Application Testing Skill

Use this skill to test, visually verify, and interact with web applications natively through OmniScope, **without relying on external browser drivers (such as Playwright, Puppeteer, or Selenium)** that require downloading multi-hundred-megabyte binaries from Microsoft/Google CDNs.

## Why Use OmniScope Instead of External Headless Drivers
- **Zero External Downloads:** External tools like Playwright frequently fail with HTTP 404 or CDN connection drops when trying to download `playwright-*-win32_x64.zip`. OmniScope uses the system's native graphics and input drivers.
- **Native OS & Browser Fidelity:** Runs inside genuine Windows browsers (Edge, Chrome, Firefox) rather than simulated or stripped headless builds.
- **Full Kernel & Network Observability:** Directly captures DNS queries, API requests, and spawned background processes alongside the visual UI.
- **Instant Rollback:** Allows rolling back cookies, local storage, sessions, and disk modifications in milliseconds between test runs.

---

## Autonomous Web Testing Workflow

### Step 1: Baseline Checkpoint
Before launching or testing the web application, create an instant snapshot:
```json
// Tool: sandbox_snapshot_create
{
  "snapshot_name": "web_test_clean"
}
```

### Step 2: Launch Browser to Target URL
Launch the installed browser inside the sandbox pointed at the local or remote target application:
```json
// Tool: sandbox_launch_app
{
  "executable_path": "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "arguments": ["http://localhost:3000", "--start-maximized"]
}
```
*(Alternatively for Chrome: `C:\Program Files\Google\Chrome\Application\chrome.exe`)*

### Step 3: Zero-Copy Screen Capture
Capture the visual frame buffer using OmniScope's native GDI BitBlt engine to verify page rendering:
```json
// Tool: sandbox_get_screen
{
  "format": "webp",
  "crop_active_window": false
}
```

### Step 4: Extract UI Controls & Coordinates
Query the Windows accessibility tree to find the exact screen coordinates of input fields, buttons, and navigation elements:
```json
// Tool: sandbox_get_ui_tree
{
  "max_depth": 4,
  "window_filter": "Edge"
}
```

### Step 5: Biometric Form Filling & Navigation
Interact with the web application using natural human-like input:
- **Click element:**
  ```json
  // Tool: sandbox_click
  {
    "x": 640,
    "y": 380,
    "button": "left"
  }
  ```
- **Type into fields with realistic timing jitter:**
  ```json
  // Tool: sandbox_type_text
  {
    "text": "test_user@example.com",
    "press_enter": false
  }
  ```

### Step 6: Monitor API Calls & Network Telemetry
Verify backend API requests, authentication calls, and outgoing traffic without noisy Windows background requests:
```json
// Tool: sandbox_query_telemetry
{
  "source": "network",
  "limit": 20
}
```

### Step 7: Clean Session Rollback
Instantly reset the browser state, clearing all session cookies, local storage tokens, and temporary files:
```json
// Tool: sandbox_snapshot_revert
{
  "snapshot_name": "web_test_clean"
}
```
