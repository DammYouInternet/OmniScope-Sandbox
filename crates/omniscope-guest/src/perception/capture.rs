use crate::perception::error::PerceptionError;
use omniscope_core::events::ImageFormat;

/// In-memory representation of an encoded or raw frame buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: Vec<u8>,
}

/// Core interface for capturing desktop visuals.
pub trait FrameGrabber: Send + Sync {
    /// Captures the desktop frame, optionally cropping to the frontmost active window.
    fn capture_screen(
        &self,
        crop_active_window: bool,
        format: ImageFormat,
        quality: u8,
    ) -> Result<RawFrame, PerceptionError>;

    /// Returns the current capture resolution (width, height).
    fn get_resolution(&self) -> (u32, u32);
}

/// Simulated frame grabber for unit testing and headless CI.
#[derive(Debug, Clone)]
pub struct MockFrameGrabber {
    pub width: u32,
    pub height: u32,
    pub fixed_payload: Option<Vec<u8>>,
}

impl MockFrameGrabber {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            fixed_payload: None,
        }
    }

    pub fn with_payload(width: u32, height: u32, payload: Vec<u8>) -> Self {
        Self {
            width,
            height,
            fixed_payload: Some(payload),
        }
    }
}

impl FrameGrabber for MockFrameGrabber {
    fn capture_screen(
        &self,
        _crop_active_window: bool,
        format: ImageFormat,
        _quality: u8,
    ) -> Result<RawFrame, PerceptionError> {
        let data = if let Some(ref payload) = self.fixed_payload {
            payload.clone()
        } else {
            // Generate synthetic test pixel pattern (e.g. 4-byte RGBA per pixel)
            let mut buf = Vec::with_capacity((self.width * self.height * 4) as usize);
            for y in 0..self.height {
                for x in 0..self.width {
                    let r = ((x * 255) / self.width.max(1)) as u8;
                    let g = ((y * 255) / self.height.max(1)) as u8;
                    let b = 128u8;
                    let a = 255u8;
                    buf.extend_from_slice(&[r, g, b, a]);
                }
            }
            buf
        };

        Ok(RawFrame {
            width: self.width,
            height: self.height,
            format,
            data,
        })
    }

    fn get_resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

// ---------------------------------------------------------------------------
// Native Windows GDI Screen Grabber
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod ffi {
    pub type HWND = usize;
    pub type HDC = usize;
    pub type HBITMAP = usize;
    pub type HGDIOBJ = usize;

    pub const SRCCOPY: u32 = 0x00CC0020;
    pub const BI_RGB: u32 = 0;
    pub const DIB_RGB_COLORS: u32 = 0;

    pub const SM_CXSCREEN: i32 = 0;
    pub const SM_CYSCREEN: i32 = 1;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct BITMAPINFOHEADER {
        pub bi_size: u32,
        pub bi_width: i32,
        pub bi_height: i32,
        pub bi_planes: u16,
        pub bi_bit_count: u16,
        pub bi_compression: u32,
        pub bi_size_image: u32,
        pub bi_x_pels_per_meter: i32,
        pub bi_y_pels_per_meter: i32,
        pub bi_clr_used: u32,
        pub bi_clr_important: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct BITMAPINFO {
        pub bmi_header: BITMAPINFOHEADER,
        pub bmi_colors: [u32; 1],
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetDesktopWindow() -> HWND;
        pub fn GetDC(hwnd: HWND) -> HDC;
        pub fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;
        pub fn GetForegroundWindow() -> HWND;
        pub fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> i32;
        pub fn GetSystemMetrics(n_index: i32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
        pub fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> HBITMAP;
        pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
        pub fn BitBlt(
            hdc: HDC,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            hdc_src: HDC,
            x1: i32,
            y1: i32,
            rop: u32,
        ) -> i32;
        pub fn GetDIBits(
            hdc: HDC,
            hbm: HBITMAP,
            start: u32,
            lines: u32,
            lpv_bits: *mut u8,
            lpbmi: *mut BITMAPINFO,
            usage: u32,
        ) -> i32;
        pub fn DeleteDC(hdc: HDC) -> i32;
        pub fn DeleteObject(ho: HGDIOBJ) -> i32;
    }
}

/// Native Windows screen grabber using GDI BitBlt.
///
/// Universally compatible across bare metal, hypervisors (QEMU/WHPX), and VM console sessions
/// without requiring 3D GPU acceleration.
#[cfg(windows)]
pub struct WindowsScreenGrabber;

#[cfg(windows)]
impl WindowsScreenGrabber {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl FrameGrabber for WindowsScreenGrabber {
    fn capture_screen(
        &self,
        crop_active_window: bool,
        format: ImageFormat,
        _quality: u8,
    ) -> Result<RawFrame, PerceptionError> {
        let (screen_w, screen_h) = self.get_resolution();
        let mut x = 0;
        let mut y = 0;
        let mut width = screen_w as i32;
        let mut height = screen_h as i32;

        if crop_active_window {
            let fg_hwnd = unsafe { ffi::GetForegroundWindow() };
            if fg_hwnd != 0 {
                let mut rect = ffi::RECT::default();
                if unsafe { ffi::GetWindowRect(fg_hwnd, &mut rect) } != 0 {
                    let win_w = rect.right - rect.left;
                    let win_h = rect.bottom - rect.top;
                    if win_w > 0 && win_h > 0 {
                        x = rect.left.max(0);
                        y = rect.top.max(0);
                        width = win_w.min(screen_w as i32 - x);
                        height = win_h.min(screen_h as i32 - y);
                    }
                }
            }
        }

        if width <= 0 || height <= 0 {
            return Err(PerceptionError::CaptureFailed(
                "Calculated capture dimensions are zero or negative".to_string(),
            ));
        }

        let desktop_hwnd = unsafe { ffi::GetDesktopWindow() };
        let screen_dc = unsafe { ffi::GetDC(desktop_hwnd) };
        if screen_dc == 0 {
            return Err(PerceptionError::GdiError("GetDC failed".to_string()));
        }

        let mem_dc = unsafe { ffi::CreateCompatibleDC(screen_dc) };
        if mem_dc == 0 {
            unsafe { ffi::ReleaseDC(desktop_hwnd, screen_dc) };
            return Err(PerceptionError::GdiError("CreateCompatibleDC failed".to_string()));
        }

        let hbitmap = unsafe { ffi::CreateCompatibleBitmap(screen_dc, width, height) };
        if hbitmap == 0 {
            unsafe {
                ffi::DeleteDC(mem_dc);
                ffi::ReleaseDC(desktop_hwnd, screen_dc);
            }
            return Err(PerceptionError::GdiError("CreateCompatibleBitmap failed".to_string()));
        }

        let old_obj = unsafe { ffi::SelectObject(mem_dc, hbitmap) };
        let blt_res = unsafe {
            ffi::BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                screen_dc,
                x,
                y,
                ffi::SRCCOPY,
            )
        };

        if blt_res == 0 {
            unsafe {
                ffi::SelectObject(mem_dc, old_obj);
                ffi::DeleteObject(hbitmap);
                ffi::DeleteDC(mem_dc);
                ffi::ReleaseDC(desktop_hwnd, screen_dc);
            }
            return Err(PerceptionError::GdiError("BitBlt screen copy failed".to_string()));
        }

        // Setup 32-bit BGRA BITMAPINFO
        let mut bmi = ffi::BITMAPINFO::default();
        bmi.bmi_header.bi_size = std::mem::size_of::<ffi::BITMAPINFOHEADER>() as u32;
        bmi.bmi_header.bi_width = width;
        bmi.bmi_header.bi_height = -height; // Top-down DIB
        bmi.bmi_header.bi_planes = 1;
        bmi.bmi_header.bi_bit_count = 32;
        bmi.bmi_header.bi_compression = ffi::BI_RGB;

        let pixel_bytes_len = (width * height * 4) as usize;
        let mut pixels = vec![0u8; pixel_bytes_len];

        let scan_lines = unsafe {
            ffi::GetDIBits(
                mem_dc,
                hbitmap,
                0,
                height as u32,
                pixels.as_mut_ptr(),
                &mut bmi,
                ffi::DIB_RGB_COLORS,
            )
        };

        // Clean up GDI handles
        unsafe {
            ffi::SelectObject(mem_dc, old_obj);
            ffi::DeleteObject(hbitmap);
            ffi::DeleteDC(mem_dc);
            ffi::ReleaseDC(desktop_hwnd, screen_dc);
        }

        if scan_lines == 0 {
            return Err(PerceptionError::GdiError("GetDIBits pixel read failed".to_string()));
        }

        // Convert BGRA from GDI to RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            let b = chunk[0];
            let r = chunk[2];
            chunk[0] = r;
            chunk[2] = b;
        }

        Ok(RawFrame {
            width: width as u32,
            height: height as u32,
            format,
            data: pixels,
        })
    }

    fn get_resolution(&self) -> (u32, u32) {
        let cx = unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) };
        let cy = unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) };
        (cx.max(1) as u32, cy.max(1) as u32)
    }
}
