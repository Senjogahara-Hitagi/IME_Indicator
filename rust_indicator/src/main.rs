#![windows_subsystem = "windows"]

mod caret_detector;
mod config;
mod cursor_detector;
mod egui_overlay_renderer;
mod event_listener;
mod ime_detector;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use overlay_shared::egui_overlay_runner::start_overlay;
use overlay_shared::windows_support::set_process_dpi_awareness;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, LoadIconW, IDI_APPLICATION};

use caret_detector::CaretDetector;
use cursor_detector::CursorDetector;
use egui_overlay_renderer::{
    CaretVisual, IndicatorOverlayApp, MouseVisual, OverlayVisualState, SharedOverlayVisualState,
    query_system_status,
};
use ime_detector::{get_indicator_state, IndicatorState};
use tray::TrayManager;

fn main() {
    set_process_dpi_awareness();

    let running = Arc::new(AtomicBool::new(true));
    let shared_state: SharedOverlayVisualState =
        Arc::new(std::sync::Mutex::new(OverlayVisualState::default()));

    {
        let detector_running = running.clone();
        let detector_state = shared_state.clone();
        thread::spawn(move || run_detector_loop(detector_running, detector_state));
    }

    {
        let overlay_state = shared_state.clone();
        thread::spawn(move || {
            let app = IndicatorOverlayApp::new(
                overlay_state,
                config::caret_size() as f32,
                config::mouse_size() as f32,
                config::caret_color_cn(),
                config::caret_color_en(),
                config::mouse_color_cn(),
                config::mouse_color_en(),
                config::caret_offset_x(),
                config::caret_offset_y(),
                config::mouse_offset_x(),
                config::mouse_offset_y(),
            );
            start_overlay(app);
        });
    }

    if config::tray_enable() {
        unsafe {
            let h_instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
            let mut icon = LoadIconW(h_instance, windows::core::PCWSTR(1 as _))
                .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap());

            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(dir) = exe_path.parent() {
                    let external_icon = dir.join("icon.png");
                    if let Some(h) = TrayManager::load_icon_from_file(&external_icon) {
                        icon = h;
                    }
                }
            }

            let tray = TrayManager::new(icon);
            tray.run_message_loop();
            tray.destroy();
        }
    } else {
        while running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(100));
        }
    }

    running.store(false, Ordering::SeqCst);
}

fn run_detector_loop(running: Arc<AtomicBool>, shared_state: SharedOverlayVisualState) {
    let mut caret_detector = CaretDetector::new();
    let cursor_detector = CursorDetector::new(config::mouse_target_cursors());
    let event_rx = event_listener::start_event_listener();

    let state_interval = Duration::from_millis(config::state_poll_interval_ms());
    let mut last_state_check_time = Instant::now();
    
    // 初始执行一次
    let mut state = get_indicator_state();
    let mut system_status = query_system_status();
    let mut caret_active = false;
    let mut mouse_active = false;

    while running.load(Ordering::SeqCst) {
        // 使用 recv_timeout。如果系统有事件（光标动、焦点变），我们立即响应。
        // 如果没有事件，我们以较长的间隔（如 50ms）进行一次保底轮询，以防某些窗口不发事件。
        let event = event_rx.recv_timeout(Duration::from_millis(50));
        
        let now = Instant::now();

        // 核心检测：获取光标位置
        let caret_pos = if config::caret_enable() {
            caret_detector.get_caret_pos()
        } else {
            None
        };

        // 状态更新：如果收到了焦点改变事件，或者到了定时检查时间
        let is_focus_event = matches!(event, Ok(event_listener::WinEvent::FocusChanged));
        if is_focus_event || now.duration_since(last_state_check_time) >= state_interval {
            state = get_indicator_state();
            system_status = query_system_status();
            let is_chinese = matches!(
                state,
                IndicatorState::ChineseCapsLockOn | IndicatorState::ChineseCapsLockOff
            );

            if config::caret_enable() {
                caret_active = caret_pos.is_some() && (is_chinese || config::caret_show_en());
            } else {
                caret_active = false;
            }

            if config::mouse_enable() {
                let target_cursor = cursor_detector.is_target_cursor();
                mouse_active = target_cursor && (is_chinese || config::mouse_show_en());
            } else {
                mouse_active = false;
            }

            last_state_check_time = now;
        }

        // 组合最终视觉状态
        let caret = if caret_active {
            caret_pos.map(|(x, y, height)| CaretVisual { x, y, height })
        } else {
            None
        };

        let mouse = if config::mouse_enable() && mouse_active {
            let mut pt = POINT::default();
            unsafe {
                if GetCursorPos(&mut pt).is_ok() {
                    Some(MouseVisual)
                } else {
                    None
                }
            }
        } else {
            None
        };

        {
            let mut visual = shared_state.lock().expect("overlay state poisoned");
            visual.indicator_state = state;
            visual.caret = caret;
            visual.mouse = mouse;
            visual.system_status = system_status.clone();
        }
    }
}
