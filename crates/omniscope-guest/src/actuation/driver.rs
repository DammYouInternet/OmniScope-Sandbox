use crate::actuation::bezier_mouse::{generate_bezier_trajectory, BezierTrajectory};
use crate::actuation::error::ActuationError;
use crate::actuation::keyboard::{plan_keystrokes, KeyAction};
use omniscope_core::events::{ButtonAction, MouseButton};
use std::sync::RwLock;

/// Core abstraction for injecting synthetic user interactions into the operating system.
pub trait InputDriver: Send + Sync {
    /// Moves the cursor to the target coordinate, optionally following a human Bézier trajectory.
    fn move_cursor(&self, target_x: i32, target_y: i32, duration_ms: u32, use_bezier: bool) -> Result<(), ActuationError>;

    /// Injects a synthetic mouse button action.
    fn click_mouse(&self, button: MouseButton, action: ButtonAction) -> Result<(), ActuationError>;

    /// Injects synthetic keystrokes with timing jitter.
    fn type_text(&self, text: &str, press_enter: bool, jitter_range_ms: (u32, u32)) -> Result<(), ActuationError>;

    /// Returns the current coordinates of the cursor.
    fn get_cursor_position(&self) -> Result<(i32, i32), ActuationError>;

    /// Returns the primary display resolution bounds (width, height).
    fn get_screen_bounds(&self) -> (i32, i32);
}

/// A simulated in-memory input driver for automated headless tests and CI verification.
#[derive(Debug)]
pub struct MockInputDriver {
    pub screen_bounds: (i32, i32),
    pub cursor_pos: RwLock<(i32, i32)>,
    pub recorded_trajectories: RwLock<Vec<BezierTrajectory>>,
    pub recorded_clicks: RwLock<Vec<(MouseButton, ButtonAction)>>,
    pub recorded_actions: RwLock<Vec<KeyAction>>,
}

impl MockInputDriver {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            screen_bounds: (width, height),
            cursor_pos: RwLock::new((0, 0)),
            recorded_trajectories: RwLock::new(Vec::new()),
            recorded_clicks: RwLock::new(Vec::new()),
            recorded_actions: RwLock::new(Vec::new()),
        }
    }
}

impl InputDriver for MockInputDriver {
    fn move_cursor(&self, target_x: i32, target_y: i32, duration_ms: u32, use_bezier: bool) -> Result<(), ActuationError> {
        let (max_x, max_y) = self.screen_bounds;
        if target_x < 0 || target_x > max_x || target_y < 0 || target_y > max_y {
            return Err(ActuationError::OutOfBounds {
                x: target_x,
                y: target_y,
                max_x,
                max_y,
            });
        }

        let start = *self.cursor_pos.read().unwrap();
        let trajectory = if use_bezier {
            generate_bezier_trajectory(start, (target_x, target_y), duration_ms, 10)
        } else {
            generate_bezier_trajectory(start, (target_x, target_y), 0, 10)
        };

        // Update cursor to destination
        *self.cursor_pos.write().unwrap() = (target_x, target_y);
        self.recorded_trajectories.write().unwrap().push(trajectory);
        Ok(())
    }

    fn click_mouse(&self, button: MouseButton, action: ButtonAction) -> Result<(), ActuationError> {
        self.recorded_clicks.write().unwrap().push((button, action));
        Ok(())
    }

    fn type_text(&self, text: &str, press_enter: bool, jitter_range_ms: (u32, u32)) -> Result<(), ActuationError> {
        let plan = plan_keystrokes(text, press_enter, jitter_range_ms);
        self.recorded_actions.write().unwrap().extend(plan.actions);
        Ok(())
    }

    fn get_cursor_position(&self) -> Result<(i32, i32), ActuationError> {
        Ok(*self.cursor_pos.read().unwrap())
    }

    fn get_screen_bounds(&self) -> (i32, i32) {
        self.screen_bounds
    }
}

// ---------------------------------------------------------------------------
// Native Windows FFI Direct Bindings (Zero third-party overhead)
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod ffi {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct POINT {
        pub x: i32,
        pub y: i32,
    }

    pub const SM_CXSCREEN: i32 = 0;
    pub const SM_CYSCREEN: i32 = 1;

    pub const INPUT_MOUSE: u32 = 0;
    pub const INPUT_KEYBOARD: u32 = 1;

    pub const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
    pub const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
    pub const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
    pub const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
    pub const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
    pub const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;

    pub const KEYEVENTF_KEYUP: u32 = 0x0002;
    pub const KEYEVENTF_UNICODE: u32 = 0x0004;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MOUSEINPUT {
        pub dx: i32,
        pub dy: i32,
        pub mouse_data: u32,
        pub dw_flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct KEYBDINPUT {
        pub w_vk: u16,
        pub w_scan: u16,
        pub dw_flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union INPUT_UNION {
        pub mi: MOUSEINPUT,
        pub ki: KEYBDINPUT,
        pub padding: [u64; 4],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct INPUT {
        pub r#type: u32,
        pub u: INPUT_UNION,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn SetCursorPos(x: i32, y: i32) -> i32;
        pub fn GetCursorPos(lp_point: *mut POINT) -> i32;
        pub fn SendInput(c_inputs: u32, p_inputs: *const INPUT, cb_size: i32) -> u32;
        pub fn GetSystemMetrics(n_index: i32) -> i32;
    }
}

/// Native Windows input driver using Windows SendInput.
#[cfg(windows)]
pub struct WindowsInputDriver;

#[cfg(windows)]
impl WindowsInputDriver {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl InputDriver for WindowsInputDriver {
    fn move_cursor(&self, target_x: i32, target_y: i32, duration_ms: u32, use_bezier: bool) -> Result<(), ActuationError> {
        let (max_x, max_y) = self.get_screen_bounds();
        if target_x < 0 || target_x > max_x || target_y < 0 || target_y > max_y {
            return Err(ActuationError::OutOfBounds {
                x: target_x,
                y: target_y,
                max_x,
                max_y,
            });
        }

        let mut pt = ffi::POINT::default();
        unsafe {
            ffi::GetCursorPos(&mut pt);
        }
        let start = (pt.x, pt.y);

        if use_bezier && duration_ms > 0 {
            let trajectory = generate_bezier_trajectory(start, (target_x, target_y), duration_ms, 8);
            for step in trajectory.steps {
                unsafe {
                    ffi::SetCursorPos(step.x, step.y);
                }
                std::thread::sleep(step.delay);
            }
        } else {
            unsafe {
                ffi::SetCursorPos(target_x, target_y);
            }
        }

        Ok(())
    }

    fn click_mouse(&self, button: MouseButton, action: ButtonAction) -> Result<(), ActuationError> {
        let (down_flag, up_flag) = match button {
            MouseButton::Left => (ffi::MOUSEEVENTF_LEFTDOWN, ffi::MOUSEEVENTF_LEFTUP),
            MouseButton::Right => (ffi::MOUSEEVENTF_RIGHTDOWN, ffi::MOUSEEVENTF_RIGHTUP),
            MouseButton::Middle => (ffi::MOUSEEVENTF_MIDDLEDOWN, ffi::MOUSEEVENTF_MIDDLEUP),
        };

        let send_flags = |flags: u32| -> Result<(), ActuationError> {
            let input = ffi::INPUT {
                r#type: ffi::INPUT_MOUSE,
                u: ffi::INPUT_UNION {
                    mi: ffi::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouse_data: 0,
                        dw_flags: flags,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            };
            let sent = unsafe { ffi::SendInput(1, &input, std::mem::size_of::<ffi::INPUT>() as i32) };
            if sent == 1 {
                Ok(())
            } else {
                Err(ActuationError::SendInputFailed(sent))
            }
        };

        match action {
            ButtonAction::Down => send_flags(down_flag),
            ButtonAction::Up => send_flags(up_flag),
            ButtonAction::Click => {
                send_flags(down_flag)?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                send_flags(up_flag)
            }
            ButtonAction::DoubleClick => {
                send_flags(down_flag)?;
                std::thread::sleep(std::time::Duration::from_millis(40));
                send_flags(up_flag)?;
                std::thread::sleep(std::time::Duration::from_millis(80));
                send_flags(down_flag)?;
                std::thread::sleep(std::time::Duration::from_millis(40));
                send_flags(up_flag)
            }
        }
    }

    fn type_text(&self, text: &str, press_enter: bool, jitter_range_ms: (u32, u32)) -> Result<(), ActuationError> {
        let plan = plan_keystrokes(text, press_enter, jitter_range_ms);

        for action in plan.actions {
            match action {
                KeyAction::KeyDown { char_code, is_unicode } => {
                    let flags = if is_unicode { ffi::KEYEVENTF_UNICODE } else { 0 };
                    let input = ffi::INPUT {
                        r#type: ffi::INPUT_KEYBOARD,
                        u: ffi::INPUT_UNION {
                            ki: ffi::KEYBDINPUT {
                                w_vk: if is_unicode { 0 } else { char_code },
                                w_scan: if is_unicode { char_code } else { 0 },
                                dw_flags: flags,
                                time: 0,
                                dw_extra_info: 0,
                            },
                        },
                    };
                    unsafe { ffi::SendInput(1, &input, std::mem::size_of::<ffi::INPUT>() as i32) };
                }
                KeyAction::KeyUp { char_code, is_unicode } => {
                    let mut flags = ffi::KEYEVENTF_KEYUP;
                    if is_unicode {
                        flags |= ffi::KEYEVENTF_UNICODE;
                    }
                    let input = ffi::INPUT {
                        r#type: ffi::INPUT_KEYBOARD,
                        u: ffi::INPUT_UNION {
                            ki: ffi::KEYBDINPUT {
                                w_vk: if is_unicode { 0 } else { char_code },
                                w_scan: if is_unicode { char_code } else { 0 },
                                dw_flags: flags,
                                time: 0,
                                dw_extra_info: 0,
                            },
                        },
                    };
                    unsafe { ffi::SendInput(1, &input, std::mem::size_of::<ffi::INPUT>() as i32) };
                }
                KeyAction::Delay(duration) => {
                    std::thread::sleep(duration);
                }
            }
        }

        Ok(())
    }

    fn get_cursor_position(&self) -> Result<(i32, i32), ActuationError> {
        let mut pt = ffi::POINT::default();
        unsafe {
            ffi::GetCursorPos(&mut pt);
        }
        Ok((pt.x, pt.y))
    }

    fn get_screen_bounds(&self) -> (i32, i32) {
        let cx = unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) };
        let cy = unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) };
        (cx, cy)
    }
}
