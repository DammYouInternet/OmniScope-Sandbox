pub mod actuation;
pub mod ipc;
pub mod perception;
pub mod telemetry;

pub use actuation::{
    bezier_mouse, driver, error as actuation_error, keyboard, ActuationError, BezierTrajectory,
    InputDriver, KeyAction, KeystrokePlan, MockInputDriver, Point, TrajectoryStep,
};
pub use ipc::GuestDispatcher;
pub use perception::{
    capture, error as perception_error, uia_tree, FrameGrabber, MockFrameGrabber,
    MockUiTreeWalker, PerceptionError, RawFrame, UiTreeWalker,
};
pub use telemetry::{
    error as telemetry_error, noise_filter, tap_manager, taps, FileSystemTap, NetworkTap,
    NoiseFilter, ProcessTap, RegistryTap, TapManager, TelemetryError,
};

#[cfg(windows)]
pub use actuation::WindowsInputDriver;
#[cfg(windows)]
pub use perception::{WindowsScreenGrabber, WindowsUiTreeWalker};

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::events::{
        ButtonAction, FileOp, FileSystemEvent, ImageFormat, MouseButton, ProcessEvent,
        ProcessEventType, RegistryEvent, RegistryOp, TelemetryEvent, TelemetrySource,
    };

    // -----------------------------------------------------------------------
    // Chunk 2: Actuation Tests
    // -----------------------------------------------------------------------
    #[test]
    fn test_bezier_trajectory_destination_exactness() {
        let start = (150, 200);
        let target = (1280, 720);
        let duration_ms = 250;
        let step_ms = 10;

        let trajectory = actuation::generate_bezier_trajectory(start, target, duration_ms, step_ms);

        assert!(!trajectory.steps.is_empty());
        assert_eq!(trajectory.destination(), target);
        assert_eq!(trajectory.steps.last().unwrap().x, target.0);
        assert_eq!(trajectory.steps.last().unwrap().y, target.1);
    }

    #[test]
    fn test_bezier_biometric_ease_in_ease_out() {
        let start = (0, 0);
        let target = (1000, 0);
        let duration_ms = 400;
        let step_ms = 10;

        let trajectory = actuation::generate_bezier_trajectory(start, target, duration_ms, step_ms);
        let steps = &trajectory.steps;
        assert!(steps.len() > 10);

        let mut velocities = Vec::new();
        let mut prev_x = start.0;
        for step in steps {
            velocities.push((step.x - prev_x).abs());
            prev_x = step.x;
        }

        let initial_velocity = velocities[0];
        let mid_idx = velocities.len() / 2;
        let peak_velocity = velocities[mid_idx];
        let terminal_velocity = *velocities.last().unwrap();

        assert!(peak_velocity > initial_velocity);
        assert!(peak_velocity > terminal_velocity);
    }

    #[test]
    fn test_keyboard_plan_generation_and_timing() {
        let text = "Hello, OmniScope!";
        let plan = actuation::plan_keystrokes(text, true, (50, 100));

        assert_eq!(plan.key_count(), 18);
        let millis = plan.total_duration.as_millis();
        assert!(millis >= 1000 && millis <= 4000);

        let key_downs = plan
            .actions
            .iter()
            .filter(|a| matches!(a, KeyAction::KeyDown { .. }))
            .count();
        let key_ups = plan
            .actions
            .iter()
            .filter(|a| matches!(a, KeyAction::KeyUp { .. }))
            .count();

        assert_eq!(key_downs, key_ups);
    }

    #[test]
    fn test_mock_driver_bounds_checking() {
        let driver = MockInputDriver::new(1920, 1080);
        assert!(driver.move_cursor(500, 400, 100, true).is_ok());
        assert_eq!(driver.get_cursor_position().unwrap(), (500, 400));
        assert!(matches!(driver.move_cursor(-5, 400, 100, true), Err(ActuationError::OutOfBounds { .. })));
        assert!(matches!(driver.move_cursor(2560, 400, 100, true), Err(ActuationError::OutOfBounds { .. })));
    }

    #[test]
    fn test_mock_driver_recording_clicks_and_keys() {
        let driver = MockInputDriver::new(1920, 1080);
        driver.click_mouse(MouseButton::Left, ButtonAction::Click).unwrap();
        driver.click_mouse(MouseButton::Right, ButtonAction::DoubleClick).unwrap();

        let clicks = driver.recorded_clicks.read().unwrap();
        assert_eq!(clicks.len(), 2);
        assert_eq!(clicks[0], (MouseButton::Left, ButtonAction::Click));
        assert_eq!(clicks[1], (MouseButton::Right, ButtonAction::DoubleClick));

        driver.type_text("Omni", false, (50, 100)).unwrap();
        let actions = driver.recorded_actions.read().unwrap();
        assert_eq!(actions.len(), 16);
    }

    // -----------------------------------------------------------------------
    // Chunk 3: Perception Tests
    // -----------------------------------------------------------------------
    #[test]
    fn test_mock_frame_grabber_dimensions_and_payload() {
        let grabber = MockFrameGrabber::new(1280, 720);
        let frame = grabber
            .capture_screen(false, ImageFormat::WebP, 80)
            .expect("Capture should succeed");

        assert_eq!(frame.width, 1280);
        assert_eq!(frame.height, 720);
        assert_eq!(frame.format, ImageFormat::WebP);
        assert_eq!(frame.data.len(), 1280 * 720 * 4);
    }

    #[test]
    fn test_mock_ui_tree_walker_hierarchy() {
        let walker = MockUiTreeWalker::new();
        let tree = walker
            .walk_tree(None, 5, None)
            .expect("Walk tree should succeed");

        assert_eq!(tree.name, "Desktop");
        assert_eq!(tree.control_type, "Desktop");
        assert!(!tree.children.is_empty());

        let target_window = tree
            .children
            .iter()
            .find(|c| c.name.contains("Target Application"))
            .expect("Target Application window must be present");

        assert_eq!(target_window.children.len(), 3);
        let button_names: Vec<&str> = target_window
            .children
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(button_names.contains(&"Connect"));
        assert!(button_names.contains(&"Export Settings"));
        assert!(button_names.contains(&"Server URL"));
    }

    #[test]
    fn test_mock_ui_tree_walker_filtering() {
        let walker = MockUiTreeWalker::new();
        let filtered_tree = walker
            .walk_tree(None, 5, Some("Target Application"))
            .expect("Walk tree with filter should succeed");

        assert_eq!(filtered_tree.children.len(), 1);
        assert!(filtered_tree.children[0].name.contains("Target Application"));
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_screen_grabber_resolution() {
        let grabber = WindowsScreenGrabber::new();
        let (w, h) = grabber.get_resolution();
        assert!(w > 0, "Screen width must be positive: {}", w);
        assert!(h > 0, "Screen height must be positive: {}", h);
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_ui_tree_walker_live() {
        let walker = WindowsUiTreeWalker::new();
        let tree_res = walker.walk_tree(None, 3, None);
        assert!(tree_res.is_ok(), "Live Windows window enumeration must succeed");
        let tree = tree_res.unwrap();
        assert_eq!(tree.name, "Desktop");
    }

    // -----------------------------------------------------------------------
    // Chunk 4: Telemetry Taps & Noise Filter Tests
    // -----------------------------------------------------------------------
    #[test]
    fn test_noise_filter_detection() {
        let filter = NoiseFilter::new();

        // Background system daemons should be detected as noise
        assert!(filter.is_noise_process("dwm.exe"));
        assert!(filter.is_noise_process("C:\\Windows\\System32\\svchost.exe"));
        assert!(filter.is_noise_process("explorer.exe"));

        // User target software should NOT be detected as noise
        assert!(!filter.is_noise_process("C:\\TargetApp\\target.exe"));
        assert!(!filter.is_noise_process("malware_sample.exe"));

        // Internal OS file paths should be detected as noise
        assert!(filter.is_noise_path("C:\\Windows\\Fonts\\arial.ttf"));
        assert!(filter.is_noise_path("C:\\Windows\\WinSxS\\manifest.xml"));

        // Target application paths should NOT be detected as noise
        assert!(!filter.is_noise_path("C:\\Users\\User\\AppData\\Local\\TargetApp\\config.json"));
        assert!(!filter.is_noise_path("C:\\Program Files\\App\\app.exe"));
    }

    #[test]
    fn test_decoupled_fs_tap_recording_and_query() {
        let tap = FileSystemTap::new(10);

        tap.record(FileSystemEvent {
            timestamp_ns: 100,
            pid: 1000,
            operation: FileOp::Create,
            file_path: "C:\\AppData\\settings.ini".to_string(),
            bytes_transferred: None,
        });

        tap.record(FileSystemEvent {
            timestamp_ns: 200,
            pid: 1000,
            operation: FileOp::Write,
            file_path: "C:\\AppData\\license.key".to_string(),
            bytes_transferred: Some(128),
        });

        tap.record(FileSystemEvent {
            timestamp_ns: 300,
            pid: 2000,
            operation: FileOp::Write,
            file_path: "C:\\Other\\unrelated.log".to_string(),
            bytes_transferred: Some(64),
        });

        // Query by PID 1000
        let pid1000_events = tap.query(Some(1000), None, 10);
        assert_eq!(pid1000_events.len(), 2);
        assert_eq!(pid1000_events[0].file_path, "C:\\AppData\\settings.ini");
        assert_eq!(pid1000_events[1].file_path, "C:\\AppData\\license.key");

        // Query by string filter "license"
        let license_events = tap.query(None, Some("license"), 10);
        assert_eq!(license_events.len(), 1);
        assert_eq!(license_events[0].file_path, "C:\\AppData\\license.key");
    }

    #[test]
    fn test_tap_manager_target_pid_filtering_and_child_adoption() {
        let manager = TapManager::new(10, 10, 10, 10);
        // Track exclusively PID 5000
        manager.configure(vec![5000], true, true, true, true);

        // Event for untracked PID 9999 -> should be ignored
        manager.record_event(TelemetryEvent::FileSystem(FileSystemEvent {
            timestamp_ns: 100,
            pid: 9999,
            operation: FileOp::Write,
            file_path: "C:\\Noise\\file.txt".to_string(),
            bytes_transferred: None,
        }));

        let (fs_events, _, _) = manager.query_slice(TelemetrySource::FileSystem, None, None, 10);
        assert_eq!(fs_events.len(), 0);

        // Event for tracked PID 5000 -> should be recorded
        manager.record_event(TelemetryEvent::FileSystem(FileSystemEvent {
            timestamp_ns: 200,
            pid: 5000,
            operation: FileOp::Write,
            file_path: "C:\\AppData\\Target\\saved.dat".to_string(),
            bytes_transferred: Some(256),
        }));

        let (fs_events, total, _) = manager.query_slice(TelemetrySource::FileSystem, None, None, 10);
        assert_eq!(fs_events.len(), 1);
        assert_eq!(total, 1);

        // PID 5000 spawns child process PID 5001 -> should be automatically adopted into tracking
        manager.record_event(TelemetryEvent::Process(ProcessEvent {
            timestamp_ns: 300,
            event_type: ProcessEventType::Start,
            pid: 5001,
            parent_pid: Some(5000),
            image_name: "helper.exe".to_string(),
            command_line: Some("helper.exe --worker".to_string()),
            dll_path: None,
        }));

        // Event from newly adopted child PID 5001 -> should now be recorded!
        manager.record_event(TelemetryEvent::Registry(RegistryEvent {
            timestamp_ns: 400,
            pid: 5001,
            operation: RegistryOp::SetValue,
            key_path: "HKCU\\Software\\TargetApp".to_string(),
            value_name: Some("Installed".to_string()),
            data_preview: Some("1".to_string()),
        }));

        let (reg_events, reg_total, _) = manager.query_slice(TelemetrySource::Registry, None, None, 10);
        assert_eq!(reg_events.len(), 1);
        assert_eq!(reg_total, 1);
    }
}
