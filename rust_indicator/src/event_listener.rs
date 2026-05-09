use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{
    SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage, OBJID_CARET,
    EVENT_OBJECT_FOCUS, EVENT_OBJECT_LOCATIONCHANGE,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

/// 事件类型
#[derive(Debug, Clone, Copy)]
pub enum WinEvent {
    FocusChanged,
    CaretMoved,
    // 可以添加更多事件，如窗口销毁等
}

/// 全局 Sender，用于回调函数中发送事件
static mut EVENT_SENDER: Option<Sender<WinEvent>> = None;

/// 启动事件监听线程
pub fn start_event_listener() -> Receiver<WinEvent> {
    let (tx, rx) = channel();
    
    // 安全起见，在单线程初始化
    unsafe {
        EVENT_SENDER = Some(tx);
    }

    thread::spawn(move || {
        unsafe {
            // 设置钩子：从焦点变化到位置变化
            // OBJID_CARET 的位置变化能捕捉到大部分原生输入框的光标移动
            let hook = SetWinEventHook(
                EVENT_OBJECT_FOCUS,
                EVENT_OBJECT_LOCATIONCHANGE,
                None,
                Some(win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );

            if hook.is_invalid() {
                eprintln!("Failed to install WinEventHook");
                return;
            }

            // 必须有消息循环才能接收到钩子回调
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let _ = UnhookWinEvent(hook);
        }
    });

    rx
}

/// WinEvent 回调函数
unsafe extern "system" fn win_event_proc(
    _h_win_event_hook: HWINEVENTHOOK,
    event: u32,
    _hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _dw_event_thread: u32,
    _dwms_event_time: u32,
) {
    // 使用 addr_of! 来避免对 static mut 的直接引用警告
    if let Some(tx) = &*std::ptr::addr_of!(EVENT_SENDER) {
        match event {
            EVENT_OBJECT_FOCUS => {
                let _ = tx.send(WinEvent::FocusChanged);
            }
            EVENT_OBJECT_LOCATIONCHANGE => {
                // 只关注光标的变化
                if id_object == OBJID_CARET.0 {
                    let _ = tx.send(WinEvent::CaretMoved);
                }
            }
            _ => {}
        }
    }
}
