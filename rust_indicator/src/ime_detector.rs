//! IME state detection.

use std::sync::Once;
use windows::core::IUnknown;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::System::Variant::VariantToUInt32;
use windows::Win32::UI::Input::Ime::{IME_CMODE_NATIVE, ImmGetDefaultIMEWnd};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, GetKeyState, VK_CAPITAL};
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, CLSID_TF_ThreadMgr,
    GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, GUID_TFCAT_TIP_KEYBOARD,
    ITfInputProcessorProfileMgr, ITfThreadMgr, TF_CONVERSIONMODE_NATIVE,
    TF_INPUTPROCESSORPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SendMessageTimeoutW,
    GUITHREADINFO, SMTO_ABORTIFHUNG,
};

const WM_IME_CONTROL: u32 = 0x0283;
const IMC_GETOPENSTATUS: usize = 0x0005;
const IMC_GETCONVERSIONMODE: usize = 0x0001;

fn get_foreground_window() -> HWND {
    unsafe { GetForegroundWindow() }
}

fn get_focused_window(foreground: HWND) -> HWND {
    unsafe {
        if foreground.0.is_null() {
            return HWND::default();
        }

        let thread_id = GetWindowThreadProcessId(foreground, None);
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

        foreground
    }
}

fn send_message_timeout(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> Option<usize> {
    unsafe {
        let mut result: usize = 0;
        let ret = SendMessageTimeoutW(
            hwnd,
            msg,
            WPARAM(wparam),
            LPARAM(lparam),
            SMTO_ABORTIFHUNG,
            500,
            Some(&mut result),
        );

        (ret.0 != 0).then_some(result)
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

fn is_english_hkl(foreground: HWND) -> bool {
    unsafe {
        let thread_id = GetWindowThreadProcessId(foreground, None);
        let layout = GetKeyboardLayout(thread_id);
        matches!(
            (layout.0 as usize) & 0xffff,
            0x0409
                | 0x0809
                | 0x0c09
                | 0x1009
                | 0x1409
                | 0x1809
                | 0x1c09
                | 0x2009
                | 0x2409
                | 0x2809
                | 0x2c09
                | 0x3009
                | 0x3409
        )
    }
}

fn query_foreground_ime_language_mode(foreground: HWND) -> Option<bool> {
    unsafe {
        let target_hwnd = get_focused_window(foreground);
        let ime_hwnd = ImmGetDefaultIMEWnd(target_hwnd);
        if ime_hwnd.0.is_null() {
            return None;
        }

        let open_status = send_message_timeout(ime_hwnd, WM_IME_CONTROL, IMC_GETOPENSTATUS, 0)?;
        if open_status == 0 {
            return Some(false);
        }

        let conversion_mode =
            send_message_timeout(ime_hwnd, WM_IME_CONTROL, IMC_GETCONVERSIONMODE, 0)?;
        Some((conversion_mode as u32 & IME_CMODE_NATIVE.0 as u32) != 0)
    }
}

fn with_foreground_thread_input_attached<T>(foreground_thread: u32, f: impl FnOnce() -> T) -> T {
    unsafe {
        let current_thread = GetCurrentThreadId();
        let attached = foreground_thread != 0
            && foreground_thread != current_thread
            && AttachThreadInput(current_thread, foreground_thread, true).as_bool();
        let result = f();
        if attached {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
        result
    }
}

fn query_tsf_language_mode(foreground: HWND) -> Option<bool> {
    static COM_INIT: Once = Once::new();

    unsafe {
        COM_INIT.call_once(|| {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        });

        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        with_foreground_thread_input_attached(foreground_thread, || {
            let profile_mgr: ITfInputProcessorProfileMgr = CoCreateInstance(
                &CLSID_TF_InputProcessorProfiles,
                None::<&IUnknown>,
                CLSCTX_INPROC_SERVER,
            )
            .ok()?;

            let mut profile = TF_INPUTPROCESSORPROFILE::default();
            profile_mgr
                .GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut profile)
                .ok()?;

            if profile.dwProfileType != TF_PROFILETYPE_INPUTPROCESSOR {
                return None;
            }

            CoCreateInstance::<_, ITfThreadMgr>(
                &CLSID_TF_ThreadMgr,
                None::<&IUnknown>,
                CLSCTX_INPROC_SERVER,
            )
            .ok()
            .and_then(|thread_mgr| {
                let _ = thread_mgr.Activate();
                thread_mgr.GetGlobalCompartment().ok()
            })
            .and_then(|mgr| {
                mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)
                    .ok()
            })
            .and_then(|compartment| compartment.GetValue().ok())
            .and_then(|variant| VariantToUInt32(&variant).ok())
            .map(|conversion| (conversion & TF_CONVERSIONMODE_NATIVE) != 0)
        })
    }
}

pub fn is_chinese_mode() -> bool {
    let foreground = get_foreground_window();
    if foreground.0.is_null() {
        return false;
    }

    if is_english_hkl(foreground) {
        return false;
    }

    query_tsf_language_mode(foreground)
        .or_else(|| query_foreground_ime_language_mode(foreground))
        .unwrap_or(false)
}

pub fn get_indicator_state() -> IndicatorState {
    match (is_chinese_mode(), is_caps_lock_on()) {
        (true, true) => IndicatorState::ChineseCapsLockOn,
        (true, false) => IndicatorState::ChineseCapsLockOff,
        (false, true) => IndicatorState::EnglishCapsLockOn,
        (false, false) => IndicatorState::EnglishCapsLockOff,
    }
}
