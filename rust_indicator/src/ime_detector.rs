//! IME 状态检测模块 - 检测中英文输入模式

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    SendMessageTimeoutW, GUITHREADINFO, SMTO_ABORTIFHUNG,
};

/// IME 控制消息
const WM_IME_CONTROL: u32 = 0x283;
const IMC_GETOPENSTATUS: usize = 0x5;
const IMC_GETCONVERSIONMODE: usize = 0x1;
const IME_CMODE_NATIVE: u32 = 0x0001;

/// 获取当前焦点窗口
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

/// 获取 IME 默认窗口句柄
fn get_ime_window(hwnd: HWND) -> HWND {
    unsafe { ImmGetDefaultIMEWnd(hwnd) }
}

/// 带超时的消息发送
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

use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL;

/// 获取 Caps Lock 状态
pub fn is_caps_lock_on() -> bool {
    unsafe {
        // GetKeyState 返回值的最低位表示切换状态 (toggle state)
        (GetKeyState(VK_CAPITAL.0 as i32) & 1) != 0
    }
}

/// 指示器状态枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndicatorState {
    ChineseCapsLockOn,   // 中文模式 + 大写锁定
    ChineseCapsLockOff,  // 中文模式 + 小写
    EnglishCapsLockOn,   // 英文模式 + 大写锁定
    EnglishCapsLockOff,  // 英文模式 + 小写
}

impl IndicatorState {
    pub fn get_text(&self) -> &str {
        match self {
            Self::ChineseCapsLockOn => "A",
            Self::ChineseCapsLockOff => "中",
            Self::EnglishCapsLockOn => "A",
            Self::EnglishCapsLockOff => "a",
        }
    }
}

/// 检测是否为中文输入模式
pub fn is_chinese_mode() -> bool {
    let hwnd = get_focused_window();
    let ime_hwnd = get_ime_window(hwnd);

    if ime_hwnd.0.is_null() {
        return false;
    }

    // 获取 IME 开启状态
    let open_status = send_message_timeout(ime_hwnd, WM_IME_CONTROL, IMC_GETOPENSTATUS, 0);
    if open_status.unwrap_or(0) == 0 {
        return false;
    }

    // 获取转换模式并检测是否包含 NATIVE 标志 (中文)
    if let Some(conversion_mode) = send_message_timeout(ime_hwnd, WM_IME_CONTROL, IMC_GETCONVERSIONMODE, 0) {   
        return (conversion_mode as u32 & IME_CMODE_NATIVE) != 0;
    }

    false
}

/// 获取当前综合状态
pub fn get_indicator_state() -> IndicatorState {
    let is_chinese = is_chinese_mode();
    let is_caps = is_caps_lock_on();

    match (is_chinese, is_caps) {
        (true, true) => IndicatorState::ChineseCapsLockOn,
        (true, false) => IndicatorState::ChineseCapsLockOff,
        (false, true) => IndicatorState::EnglishCapsLockOn,
        (false, false) => IndicatorState::EnglishCapsLockOff,
    }
}

