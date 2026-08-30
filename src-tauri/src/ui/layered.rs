//! Layered overlay window, shared by the pill and the subtitle strip.
//!
//! The compositor blends a premultiplied buffer straight onto the screen, so an
//! overlay has no window background to flash through and keeps its rounded
//! shape. Everything here runs on the thread that created the window: Windows
//! only lets that thread destroy it.

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
    GetMonitorInfoW, HBITMAP, HDC, HGDIOBJ, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromPoint, ReleaseDC, SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, MSG, PM_REMOVE,
    PeekMessageW, RegisterClassW, SW_SHOWNOACTIVATE, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows_sys::core::PCWSTR;

/// Windows reports DPI against this baseline; the ratio is our render scale.
const BASELINE_DPI: f32 = 96.0;

/// Scale and bounds of the display holding the cursor, which is where the user
/// is working — the same rule the macOS side follows.
///
/// # Safety
/// Calls into user32 and gdi32; safe to call from any thread.
pub(crate) unsafe fn cursor_display() -> (f32, RECT) {
    let mut cursor = POINT { x: 0, y: 0 };
    GetCursorPos(&mut cursor);
    let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);

    let mut dpi_x = BASELINE_DPI as u32;
    let mut dpi_y = 0u32;
    GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(monitor, &mut info);
    (dpi_x as f32 / BASELINE_DPI, info.rcMonitor)
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, w, l)
}

pub(crate) struct Overlay {
    hwnd: HWND,
    surface: Surface,
    origin: POINT,
    size: SIZE,
}

impl Overlay {
    /// # Safety
    /// Must be called from the thread that will own the window for its lifetime.
    pub(crate) unsafe fn new(class_name: PCWSTR, x: i32, y: i32, width: i32, height: i32) -> Option<Self> {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(wnd_proc);
        class.hInstance = hinstance;
        class.lpszClassName = class_name;
        // Reopening re-registers the same class; the duplicate is refused and
        // the original registration stays valid, which is all we need.
        RegisterClassW(&class);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name,
            class_name,
            WS_POPUP,
            x,
            y,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            log::error!("Overlay: CreateWindowExW failed");
            return None;
        }

        let Some(surface) = Surface::new(width, height) else {
            log::error!("Overlay: CreateDIBSection failed");
            DestroyWindow(hwnd);
            return None;
        };
        Some(Self {
            hwnd,
            surface,
            origin: POINT { x, y },
            size: SIZE { cx: width, cy: height },
        })
    }

    /// # Safety
    /// Must run on the owning thread.
    pub(crate) unsafe fn show(&self) {
        ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
    }

    /// Drain the queue so the window stays responsive to the system.
    ///
    /// # Safety
    /// Must run on the owning thread.
    pub(crate) unsafe fn pump(&self) {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE) != 0 {
            DispatchMessageW(&msg);
        }
    }

    /// Paint `rgba`, resizing the window if the buffer's shape changed. `anchor`
    /// is the y the window's *top* edge keeps, so a strip that grows downwards
    /// stays put where it hangs from.
    ///
    /// # Safety
    /// Must run on the owning thread; `rgba` must hold `width * height * 4` bytes.
    pub(crate) unsafe fn present(&mut self, rgba: &[u8], width: i32, height: i32, x: i32, anchor: i32) {
        if width != self.size.cx || height != self.size.cy {
            let Some(surface) = Surface::new(width, height) else {
                log::error!("Overlay: resize to {width}x{height} failed");
                return;
            };
            self.surface = surface;
            self.size = SIZE { cx: width, cy: height };
        }
        self.origin = POINT { x, y: anchor };
        self.surface.copy_from(rgba);

        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let src = POINT { x: 0, y: 0 };
        UpdateLayeredWindow(
            self.hwnd,
            std::ptr::null_mut(),
            &self.origin,
            &self.size,
            self.surface.dc,
            &src,
            0,
            &blend,
            ULW_ALPHA,
        );
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd) };
    }
}

/// The DIB the compositor reads from, kept between frames so each one costs a
/// copy rather than an allocation.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u8,
    len: usize,
}

impl Surface {
    unsafe fn new(width: i32, height: i32) -> Option<Self> {
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        // Negative: our buffer starts at the top row, GDI defaults to bottom-up.
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;

        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        ReleaseDC(std::ptr::null_mut(), screen);
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let previous = SelectObject(dc, bitmap);
        let len = width as usize * height as usize * 4;
        Some(Self { dc, bitmap, previous, bits: bits.cast(), len })
    }

    unsafe fn copy_from(&mut self, rgba: &[u8]) {
        let dst = std::slice::from_raw_parts_mut(self.bits, self.len.min(rgba.len()));
        // Both sides are premultiplied; only the channel order differs.
        for (out, px) in dst.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
            out[0] = px[2];
            out[1] = px[1];
            out[2] = px[0];
            out[3] = px[3];
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}
