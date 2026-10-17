---
name: omniscope
description: Autonomous dynamic introspection, malware analysis, UI automation, and Ring -1 hypervisor snapshot rollback sandbox.
---

# OmniScope Autonomous Sandbox Skill

Use OmniScope to safely execute, observe, interact with, and dismantle untrusted or dynamic Windows software inside an isolated, hardware-assisted sandbox.

## When to Activate
Activate this skill whenever the user asks to:
- Dynamically analyze or detonate an executable, installer, or script.
- Automate UI interactions (clicks, keystrokes, form fills) inside an isolated guest environment.
- Capture visual frames or inspect native Windows accessibility UI trees.
- Monitor real-time process creation, filesystem writes, registry modifications, or network traffic without noisy OS background clutter.
- Take instant VM snapshots and cleanly revert state back to baseline.

---

## Tool Playbook & Standard Workflow

Follow the **10-Step Deterministic Introspection Workflow**:

1. **Verify Sandbox Health:**
   Call `sandbox_status` to ensure the hypervisor is active and the guest agent is connected.

2. **Freeze Baseline Snapshot:**
   Call `sandbox_snapshot_create(snapshot_name="baseline_clean")` before any unverified software is launched.

3. **Launch Application Under Test:**
   Call `sandbox_launch_app(executable_path="C:\\Target\\App.exe")`. This automatically attaches kernel taps to the target PID and its child processes.

4. **Perceive Visual Frame & UI Elements:**
   - Call `sandbox_get_screen(format="webp")` to capture the graphical display.
   - Call `sandbox_get_ui_tree(max_depth=3)` to extract interactive buttons, text fields, and control bounds.

5. **Actuate with Natural Biometrics:**
   - Call `sandbox_click(x=..., y=..., button="left")` (interpolates cubic Bézier curves to evade bot detection).
   - Call `sandbox_type_text(text="...", press_enter=true)` (injects natural keystroke jitter).

6. **Harvest Filtered Behavioral Telemetry:**
   - Call `sandbox_query_telemetry(source="filesystem", limit=20)` to see created/modified files.
   - Call `sandbox_query_telemetry(source="registry", limit=20)` to check persistence keys.
   - Call `sandbox_query_telemetry(source="network", limit=20)` to identify C2 beacons or DNS queries.
   - Call `sandbox_query_telemetry(source="process", limit=20)` to track spawned sub-processes.

7. **Inspect In-Memory State:**
   Call `sandbox_inspect_memory(pid=..., query_type="strings")` to uncover unpacked payloads or URLs.

8. **Instant Clean Rollback:**
   Call `sandbox_snapshot_revert(snapshot_name="baseline_clean")` to restore the guest OS and RAM back to pristine state in milliseconds.
