use crate::perception::error::PerceptionError;
use omniscope_core::events::UiElementNode;

/// Core interface for inspecting and traversing the OS accessibility and UI window tree.
pub trait UiTreeWalker: Send + Sync {
    /// Recursively traverses the visible UI tree down to `max_depth`.
    fn walk_tree(
        &self,
        root_hwnd: Option<u64>,
        max_depth: u32,
        filter: Option<&str>,
    ) -> Result<UiElementNode, PerceptionError>;
}

/// Simulated UI tree walker for headless tests and verification.
#[derive(Debug, Clone)]
pub struct MockUiTreeWalker {
    pub mock_root: Option<UiElementNode>,
}

impl MockUiTreeWalker {
    pub fn new() -> Self {
        Self { mock_root: None }
    }

    pub fn with_tree(root: UiElementNode) -> Self {
        Self {
            mock_root: Some(root),
        }
    }

    /// Generates a realistic mock desktop accessibility tree representing a target app session.
    pub fn create_sample_desktop_tree() -> UiElementNode {
        UiElementNode {
            id: "root-desktop".to_string(),
            control_type: "Desktop".to_string(),
            name: "Desktop".to_string(),
            bounds: [0, 0, 1920, 1080],
            is_enabled: true,
            is_focused: false,
            children: vec![
                UiElementNode {
                    id: "win-target-app".to_string(),
                    control_type: "Window".to_string(),
                    name: "Target Application v1.0".to_string(),
                    bounds: [100, 100, 800, 600],
                    is_enabled: true,
                    is_focused: true,
                    children: vec![
                        UiElementNode {
                            id: "btn-connect".to_string(),
                            control_type: "Button".to_string(),
                            name: "Connect".to_string(),
                            bounds: [120, 140, 100, 32],
                            is_enabled: true,
                            is_focused: false,
                            children: vec![],
                        },
                        UiElementNode {
                            id: "edit-server-url".to_string(),
                            control_type: "Edit".to_string(),
                            name: "Server URL".to_string(),
                            bounds: [240, 140, 300, 32],
                            is_enabled: true,
                            is_focused: true,
                            children: vec![],
                        },
                        UiElementNode {
                            id: "btn-export-settings".to_string(),
                            control_type: "Button".to_string(),
                            name: "Export Settings".to_string(),
                            bounds: [120, 200, 140, 32],
                            is_enabled: true,
                            is_focused: false,
                            children: vec![],
                        },
                    ],
                },
                UiElementNode {
                    id: "taskbar".to_string(),
                    control_type: "Pane".to_string(),
                    name: "Taskbar".to_string(),
                    bounds: [0, 1040, 1920, 40],
                    is_enabled: true,
                    is_focused: false,
                    children: vec![],
                },
            ],
        }
    }
}

impl Default for MockUiTreeWalker {
    fn default() -> Self {
        Self::new()
    }
}

impl UiTreeWalker for MockUiTreeWalker {
    fn walk_tree(
        &self,
        _root_hwnd: Option<u64>,
        _max_depth: u32,
        filter: Option<&str>,
    ) -> Result<UiElementNode, PerceptionError> {
        let root = if let Some(ref r) = self.mock_root {
            r.clone()
        } else {
            Self::create_sample_desktop_tree()
        };

        if let Some(query) = filter {
            let query_lower = query.to_lowercase();
            // Filter top-level children matching query
            let filtered_children: Vec<UiElementNode> = root
                .children
                .into_iter()
                .filter(|c| c.name.to_lowercase().contains(&query_lower))
                .collect();

            Ok(UiElementNode {
                id: root.id,
                control_type: root.control_type,
                name: root.name,
                bounds: root.bounds,
                is_enabled: root.is_enabled,
                is_focused: root.is_focused,
                children: filtered_children,
            })
        } else {
            Ok(root)
        }
    }
}

// ---------------------------------------------------------------------------
// Native Windows Window Hierarchy Walker
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod ffi {
    pub type HWND = usize;
    pub type BOOL = i32;
    pub type LPARAM = isize;
    pub type WNDENUMPROC = unsafe extern "system" fn(HWND, LPARAM) -> BOOL;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn EnumWindows(lp_enum_func: WNDENUMPROC, lp_param: LPARAM) -> BOOL;
        pub fn IsWindowVisible(hwnd: HWND) -> BOOL;
        pub fn IsWindowEnabled(hwnd: HWND) -> BOOL;
        pub fn GetWindowRect(hwnd: HWND, lp_rect: *mut RECT) -> BOOL;
        pub fn GetWindowTextW(hwnd: HWND, lp_string: *mut u16, n_max_count: i32) -> i32;
        pub fn GetClassNameW(hwnd: HWND, lp_class_name: *mut u16, n_max_count: i32) -> i32;
        pub fn GetForegroundWindow() -> HWND;
    }
}

/// Native Windows UI tree walker that enumerates active windows and their control boundaries.
#[cfg(windows)]
pub struct WindowsUiTreeWalker;

#[cfg(windows)]
impl WindowsUiTreeWalker {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl UiTreeWalker for WindowsUiTreeWalker {
    fn walk_tree(
        &self,
        _root_hwnd: Option<u64>,
        _max_depth: u32,
        filter: Option<&str>,
    ) -> Result<UiElementNode, PerceptionError> {
        struct EnumContext {
            windows: Vec<UiElementNode>,
            fg_hwnd: ffi::HWND,
        }

        unsafe extern "system" fn enum_proc(hwnd: ffi::HWND, lparam: ffi::LPARAM) -> ffi::BOOL {
            let ctx = &mut *(lparam as *mut EnumContext);

            if ffi::IsWindowVisible(hwnd) == 0 {
                return 1; // Continue
            }

            let mut title_buf = [0u16; 256];
            let len = ffi::GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 256);

            let mut class_buf = [0u16; 256];
            let class_len = ffi::GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
            let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);

            let title = if len > 0 {
                String::from_utf16_lossy(&title_buf[..len as usize])
            } else {
                class_name.clone()
            };

            let mut rect = ffi::RECT::default();
            ffi::GetWindowRect(hwnd, &mut rect);

            let is_enabled = ffi::IsWindowEnabled(hwnd) != 0;
            let is_focused = hwnd == ctx.fg_hwnd;

            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;

            if w > 0 && h > 0 {
                ctx.windows.push(UiElementNode {
                    id: format!("{:#X}", hwnd),
                    control_type: class_name,
                    name: title,
                    bounds: [rect.left, rect.top, w, h],
                    is_enabled,
                    is_focused,
                    children: Vec::new(),
                });
            }

            1 // Continue enumeration
        }

        let fg_hwnd = unsafe { ffi::GetForegroundWindow() };
        let mut ctx = EnumContext {
            windows: Vec::new(),
            fg_hwnd,
        };

        unsafe {
            ffi::EnumWindows(enum_proc, &mut ctx as *mut EnumContext as ffi::LPARAM);
        }

        if let Some(query) = filter {
            let query_lower = query.to_lowercase();
            ctx.windows
                .retain(|w| w.name.to_lowercase().contains(&query_lower));
        }

        Ok(UiElementNode {
            id: "desktop-root".to_string(),
            control_type: "Desktop".to_string(),
            name: "Desktop".to_string(),
            bounds: [0, 0, 1920, 1080],
            is_enabled: true,
            is_focused: false,
            children: ctx.windows,
        })
    }
}
