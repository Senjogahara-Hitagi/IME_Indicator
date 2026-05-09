#[cfg(target_os = "windows")]
pub fn get_virtual_desktop_rect() -> (i32, i32, i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        (x, y, width.max(1), height.max(1))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_virtual_desktop_rect() -> (i32, i32, i32, i32) {
    (0, 0, 1920, 1080)
}

#[cfg(target_os = "windows")]
pub fn get_monitor_work_rect_for_point(x: i32, y: i32) -> (i32, i32, i32, i32) {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return get_virtual_desktop_rect();
        }

        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };

        if GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _).as_bool() {
            let rect = info.rcWork;
            (
                rect.left,
                rect.top,
                (rect.right - rect.left).max(1),
                (rect.bottom - rect.top).max(1),
            )
        } else {
            get_virtual_desktop_rect()
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_monitor_work_rect_for_point(_x: i32, _y: i32) -> (i32, i32, i32, i32) {
    get_virtual_desktop_rect()
}

#[cfg(target_os = "windows")]
pub fn set_process_dpi_awareness() {
    unsafe {
        let shcore = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!(
            "shcore.dll"
        ));
        if let Ok(h) = shcore {
            if let Some(func) = windows::Win32::System::LibraryLoader::GetProcAddress(
                h,
                windows::core::s!("SetProcessDpiAwareness"),
            ) {
                let set_dpi: extern "system" fn(i32) -> i32 = std::mem::transmute(func);
                let _ = set_dpi(2);
                return;
            }
        }

        let user32 = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!(
            "user32.dll"
        ));
        if let Ok(h) = user32 {
            if let Some(func) = windows::Win32::System::LibraryLoader::GetProcAddress(
                h,
                windows::core::s!("SetProcessDPIAware"),
            ) {
                let set_dpi: extern "system" fn() -> i32 = std::mem::transmute(func);
                let _ = set_dpi();
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_process_dpi_awareness() {}

#[cfg(target_os = "windows")]
pub fn apply_overlay_window_style(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOACTIVATE, SWP_NOSIZE, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    };

    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32
            | WS_EX_LAYERED.0
            | WS_EX_TRANSPARENT.0
            | WS_EX_TOPMOST.0
            | WS_EX_NOACTIVATE.0
            | WS_EX_TOOLWINDOW.0;
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style as isize);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply_overlay_window_style(_hwnd: ()) {}
