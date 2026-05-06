//! IME state detection.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SendMessageTimeoutW,
    GUITHREADINFO, SMTO_ABORTIFHUNG,
};

const WM_IME_CONTROL: u32 = 0x283;
const IMC_GETOPENSTATUS: usize = 0x5;
const IMC_GETCONVERSIONMODE: usize = 0x1;
const IME_CMODE_NATIVE: u32 = 0x0001;

fn get_focused_window() -> HWND {
    unsafe {
        let fore_hwnd = GetForegroundWindow();
        if fore_hwnd.0.is_null() {
            return HWND::default();
        }

        let thread_id = GetWindowThreadProcessId(fore_hwnd, None);
        let mut gui_info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };

        if GetGUIThreadInfo(thread_id, &mut gui_info).is_ok() {
            if !gui_info.hwndFocus.0.is_null() {
                return gui_info.hwndFocus;
            }
            if !gui_info.hwndActive.0.is_null() {
                return gui_info.hwndActive;
            }
        }

        fore_hwnd
    }
}

fn get_ime_window(hwnd: HWND) -> HWND {
    unsafe { ImmGetDefaultIMEWnd(hwnd) }
}

fn send_message_timeout(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> Option<usize> {
    unsafe {
        let mut result: usize = 0;
        let ret = SendMessageTimeoutW(
            hwnd,
            msg,
            windows::Win32::Foundation::WPARAM(wparam),
            windows::Win32::Foundation::LPARAM(lparam),
            SMTO_ABORTIFHUNG,
            500,
            Some(&mut result),
        );

        if ret.0 != 0 {
            Some(result)
        } else {
            None
        }
    }
}

pub fn is_caps_lock_on() -> bool {
    unsafe { (GetKeyState(VK_CAPITAL.0 as i32) & 1) != 0 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndicatorState {
    ChineseCapsLockOn,
    ChineseCapsLockOff,
    EnglishCapsLockOn,
    EnglishCapsLockOff,
}

impl IndicatorState {
    pub fn is_chinese(&self) -> bool {
        matches!(self, Self::ChineseCapsLockOn | Self::ChineseCapsLockOff)
    }

    pub fn get_text(&self) -> &str {
        match self {
            Self::ChineseCapsLockOn => "A",
            Self::ChineseCapsLockOff => "\u{4E2D}",
            Self::EnglishCapsLockOn => "A",
            Self::EnglishCapsLockOff => "a",
        }
    }
}

pub fn is_chinese_mode() -> bool {
    let hwnd = get_focused_window();
    let ime_hwnd = get_ime_window(hwnd);

    if ime_hwnd.0.is_null() {
        return false;
    }

    let open_status = send_message_timeout(ime_hwnd, WM_IME_CONTROL, IMC_GETOPENSTATUS, 0);
    if open_status.unwrap_or(0) == 0 {
        return false;
    }

    if let Some(conversion_mode) =
        send_message_timeout(ime_hwnd, WM_IME_CONTROL, IMC_GETCONVERSIONMODE, 0)
    {
        return (conversion_mode as u32 & IME_CMODE_NATIVE) != 0;
    }

    false
}

pub fn get_indicator_state() -> IndicatorState {
    match (is_chinese_mode(), is_caps_lock_on()) {
        (true, true) => IndicatorState::ChineseCapsLockOn,
        (true, false) => IndicatorState::ChineseCapsLockOff,
        (false, true) => IndicatorState::EnglishCapsLockOn,
        (false, false) => IndicatorState::EnglishCapsLockOff,
    }
}
