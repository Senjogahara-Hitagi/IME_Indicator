#![windows_subsystem = "windows"]

mod caret_detector;
mod config;
mod cursor_detector;
mod egui_overlay_renderer;
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

use caret_detector::{CaretDetector, DetectionSource};
use cursor_detector::CursorDetector;
use egui_overlay_renderer::{
    CaretVisual, IndicatorOverlayApp, MouseVisual, OverlayVisualState, SharedOverlayVisualState,
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

    let state_interval = Duration::from_millis(config::state_poll_interval_ms());
    let track_interval = Duration::from_millis(config::track_poll_interval_ms());

    let mut last_state_check_time = Instant::now();
    let mut state = IndicatorState::EnglishCapsLockOff;
    let mut caret_active = false;
    let mut mouse_active = false;

    while running.load(Ordering::SeqCst) {
        let now = Instant::now();

        if now.duration_since(last_state_check_time) >= state_interval {
            state = get_indicator_state();
            let is_chinese = matches!(
                state,
                IndicatorState::ChineseCapsLockOn | IndicatorState::ChineseCapsLockOff
            );

            if config::caret_enable() {
                let caret_pos = caret_detector.get_caret_pos();
                let has_real_caret = caret_pos.is_some()
                    && !matches!(caret_detector.last_source, DetectionSource::CursorFallback);
                caret_active = has_real_caret && (is_chinese || config::caret_show_en());
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

        let caret = if config::caret_enable() && caret_active {
            caret_detector
                .get_caret_pos()
                .map(|(x, y, height)| CaretVisual { x, y, height })
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
        }

        thread::sleep(track_interval);
    }
}
