
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
    TranslateMessage, CW_USEDEFAULT, HICON, MSG, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    WM_COMMAND, WM_DESTROY, WM_RBUTTONUP, WM_USER, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP,
};
use windows::Win32::Graphics::GdiPlus::{
    GdipCreateBitmapFromFile, GdipCreateHICONFromBitmap, GdipDisposeImage,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use std::path::Path;

const WM_TRAYICON: u32 = WM_USER + 1;
const IDM_RESTART: u32 = 1001;
const IDM_CONFIG: u32 = 1002;
const IDM_ABOUT: u32 = 1003;
const IDM_EXIT: u32 = 1004;

pub struct TrayManager {
    hwnd: HWND,
}

impl TrayManager {
    pub fn new(icon: HICON) -> Self {
        unsafe {
            let h_instance = GetModuleHandleW(None).unwrap();
            let class_name = w!("IMETrayWindowClass");

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: h_instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };

            RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                Default::default(),
                class_name,
                w!("IME Indicator Tray"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                h_instance,
                None,
            ).unwrap();

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: icon,
                ..Default::default()
            };
            
            // 设置提示文字
            let tip = w!("输入指示器 (IME Indicator)");
            let tip_slice = tip.as_wide();
            let len = tip_slice.len().min(nid.szTip.len() - 1);
            nid.szTip[..len].copy_from_slice(&tip_slice[..len]);

            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            Self { hwnd }
        }
    }

    pub fn load_icon_from_file(path: &Path) -> Option<HICON> {
        unsafe {
            let path_str = path.to_str()?.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
            let mut bitmap = std::ptr::null_mut();
            
            if GdipCreateBitmapFromFile(PCWSTR(path_str.as_ptr()), &mut bitmap).0 == 0 {
                let mut hicon = HICON::default();
                if GdipCreateHICONFromBitmap(bitmap, &mut hicon).0 == 0 {
                    let _ = GdipDisposeImage(bitmap as _);
                    return Some(hicon);
                }
                let _ = GdipDisposeImage(bitmap as _);
            }
            None
        }
    }

    pub fn run_message_loop(&self) {
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    pub fn destroy(&self) {
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            DestroyWindow(self.hwnd).unwrap();
        }
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            match lparam.0 as u32 {
                WM_RBUTTONUP => {
                    show_context_menu(hwnd);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
        WM_COMMAND => {
            let id = wparam.0 as u32;
            match id {
                IDM_EXIT => {
                    std::process::exit(0);
                }
                IDM_RESTART => {
                    restart_app();
                    std::process::exit(0);
                }
                IDM_CONFIG => {
                    open_config();
                }
                IDM_ABOUT => {
                    show_about();
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_context_menu(hwnd: HWND) {
    let menu = CreatePopupMenu().unwrap();
    let _ = windows::Win32::UI::WindowsAndMessaging::AppendMenuW(
        menu,
        windows::Win32::UI::WindowsAndMessaging::MF_STRING,
        IDM_CONFIG as usize,
        w!("编辑配置 (Config)"),
    );
    let _ = windows::Win32::UI::WindowsAndMessaging::AppendMenuW(
        menu,
        windows::Win32::UI::WindowsAndMessaging::MF_STRING,
        IDM_RESTART as usize,
        w!("重启程序 (Restart)"),
    );
    let _ = windows::Win32::UI::WindowsAndMessaging::AppendMenuW(
        menu,
        windows::Win32::UI::WindowsAndMessaging::MF_STRING,
        IDM_ABOUT as usize,
        w!("关于 (About)"),
    );
    let _ = windows::Win32::UI::WindowsAndMessaging::AppendMenuW(
        menu,
        windows::Win32::UI::WindowsAndMessaging::MF_SEPARATOR,
        0,
        None,
    );
    let _ = windows::Win32::UI::WindowsAndMessaging::AppendMenuW(
        menu,
        windows::Win32::UI::WindowsAndMessaging::MF_STRING,
        IDM_EXIT as usize,
        w!("退出 (Exit)"),
    );

    let mut pos = windows::Win32::Foundation::POINT::default();
    GetCursorPos(&mut pos).unwrap();

    // 必须设置前台窗口，否则菜单点击外部不会消失
    let _ = SetForegroundWindow(hwnd);
    
    TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN,
        pos.x,
        pos.y,
        0,
        hwnd,
        None,
    ).unwrap();
    
    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(menu);
}

fn open_config() {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
        
        let path = crate::config::get_config_path();
        let path_w = path.to_str().unwrap().encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(path_w.as_ptr()),
            None,
            None,
            SW_SHOW,
        );
    }
}

fn show_about() {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
        let about_content = include_str!("../assets/about.txt");
        let about_w: Vec<u16> = about_content.encode_utf16().chain(std::iter::once(0)).collect();
        let title = w!("关于 输入指示器");
        MessageBoxW(None, PCWSTR(about_w.as_ptr()), title, MB_ICONINFORMATION | MB_OK);
    }
}

fn restart_app() {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
        use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
        
        let mut path = [0u16; 512];
        let len = GetModuleFileNameW(None, &mut path);
        if len > 0 {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(path.as_ptr()),
                None,
                None,
                SW_SHOW,
            );
        }
    }
}
