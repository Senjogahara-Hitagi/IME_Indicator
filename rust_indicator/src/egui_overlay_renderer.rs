use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Vec2};
use egui_render_three_d::ThreeDBackend as DefaultGfxBackend;

use crate::ime_detector::IndicatorState;

const WINDOW_TITLE: &str = "IME-Indicator-egui";
const PADDING_X: f32 = 6.0;
const PADDING_Y: f32 = 2.0;
const CORNER_RADIUS: f32 = 8.0;
const MIN_BUBBLE_WIDTH: f32 = 24.0;
const MIN_BUBBLE_HEIGHT: f32 = 18.0;
const CARET_MIN_ANCHOR_HEIGHT: f32 = 20.0;
const CARET_GAP_Y: f32 = 2.0;
const MOUSE_X_ANCHOR_RATIO: f32 = 0.75;

fn background_color() -> Color32 {
    Color32::from_rgba_unmultiplied(12, 18, 22, 170)
}

fn color32_from_argb(argb: u32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        ((argb >> 16) & 0xff) as u8,
        ((argb >> 8) & 0xff) as u8,
        (argb & 0xff) as u8,
        ((argb >> 24) & 0xff) as u8,
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CaretVisual {
    pub x: i32,
    pub y: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MouseVisual {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct OverlayVisualState {
    pub indicator_state: IndicatorState,
    pub caret: Option<CaretVisual>,
    pub mouse: Option<MouseVisual>,
}

impl Default for OverlayVisualState {
    fn default() -> Self {
        Self {
            indicator_state: IndicatorState::EnglishCapsLockOff,
            caret: None,
            mouse: None,
        }
    }
}

pub type SharedOverlayVisualState = Arc<Mutex<OverlayVisualState>>;

pub struct IndicatorOverlayApp {
    fonts_loaded: bool,
    window_positioned: bool,
    virtual_origin_x: f32,
    virtual_origin_y: f32,
    shared_state: SharedOverlayVisualState,
    caret_font_px: f32,
    mouse_font_px: f32,
    caret_color_cn: Color32,
    caret_color_en: Color32,
    mouse_color_cn: Color32,
    mouse_color_en: Color32,
    caret_offset_x: f32,
    caret_offset_y: f32,
    mouse_offset_x: f32,
    mouse_offset_y: f32,
}

impl IndicatorOverlayApp {
    pub fn new(
        shared_state: SharedOverlayVisualState,
        caret_font_px: f32,
        mouse_font_px: f32,
        caret_color_cn: u32,
        caret_color_en: u32,
        mouse_color_cn: u32,
        mouse_color_en: u32,
        caret_offset_x: i32,
        caret_offset_y: i32,
        mouse_offset_x: i32,
        mouse_offset_y: i32,
    ) -> Self {
        Self {
            fonts_loaded: false,
            window_positioned: false,
            virtual_origin_x: 0.0,
            virtual_origin_y: 0.0,
            shared_state,
            caret_font_px,
            mouse_font_px,
            caret_color_cn: color32_from_argb(caret_color_cn),
            caret_color_en: color32_from_argb(caret_color_en),
            mouse_color_cn: color32_from_argb(mouse_color_cn),
            mouse_color_en: color32_from_argb(mouse_color_en),
            caret_offset_x: caret_offset_x as f32,
            caret_offset_y: caret_offset_y as f32,
            mouse_offset_x: mouse_offset_x as f32,
            mouse_offset_y: mouse_offset_y as f32,
        }
    }

    fn load_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded {
            return;
        }

        let mut fonts = FontDefinitions::default();
        let font_candidates = [
            r"C:\Windows\Fonts\seguisym.ttf",
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\simsun.ttc",
        ];

        for path in font_candidates {
            if let Ok(bytes) = std::fs::read(path) {
                let name = std::path::Path::new(path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                fonts
                    .font_data
                    .insert(name.clone(), FontData::from_owned(bytes));
                fonts
                    .families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .push(name.clone());
                fonts
                    .families
                    .entry(FontFamily::Monospace)
                    .or_default()
                    .push(name);
            }
        }

        ctx.set_fonts(fonts);
        self.fonts_loaded = true;
    }

    fn draw_indicator(
        &self,
        painter: &egui::Painter,
        anchor_x: f32,
        anchor_y: f32,
        anchor_height: f32,
        font_px: f32,
        offset_x: f32,
        offset_y: f32,
        state: IndicatorState,
        mouse_mode: bool,
    ) {
        let color = if mouse_mode {
            if state.is_chinese() {
                self.mouse_color_cn
            } else {
                self.mouse_color_en
            }
        } else if state.is_chinese() {
            self.caret_color_cn
        } else {
            self.caret_color_en
        };
        let text = state.get_text();
        let font = FontId::proportional(font_px);
        let galley = painter.layout_no_wrap(text.to_string(), font, color);
        let bubble_width = (galley.rect.width() + PADDING_X * 2.0).max(MIN_BUBBLE_WIDTH);
        let bubble_height = (galley.rect.height() + PADDING_Y * 2.0).max(MIN_BUBBLE_HEIGHT);

        let pos = if mouse_mode {
            Pos2::new(
                anchor_x + offset_x - bubble_width * MOUSE_X_ANCHOR_RATIO,
                anchor_y + offset_y - bubble_height * 0.5,
            )
        } else {
            Pos2::new(
                anchor_x + offset_x - bubble_width * 0.5,
                anchor_y + anchor_height.max(CARET_MIN_ANCHOR_HEIGHT) + offset_y + CARET_GAP_Y,
            )
        };

        let rect = Rect::from_min_size(pos, Vec2::new(bubble_width, bubble_height));
        painter.rect_filled(rect, CORNER_RADIUS, background_color());
        let text_pos = Pos2::new(
            rect.center().x - galley.rect.width() * 0.5,
            rect.center().y - galley.rect.height() * 0.5,
        );
        painter.galley(text_pos, galley, color);
    }

    fn screen_px_to_overlay_point(
        &self,
        screen_x: f32,
        screen_y: f32,
        pixels_per_point: f32,
    ) -> Pos2 {
        Pos2::new(
            (screen_x - self.virtual_origin_x) / pixels_per_point,
            (screen_y - self.virtual_origin_y) / pixels_per_point,
        )
    }
}

impl egui_overlay::EguiOverlay for IndicatorOverlayApp {
    fn gui_run(
        &mut self,
        ctx: &egui::Context,
        _default_gfx_backend: &mut DefaultGfxBackend,
        glfw_backend: &mut egui_window_glfw_passthrough::GlfwBackend,
    ) {
        self.load_fonts(ctx);
        let pixels_per_point = ctx.pixels_per_point().max(1.0);

        if !self.window_positioned {
            let (x, y, w, h) = get_virtual_desktop_rect();
            glfw_backend.set_title(WINDOW_TITLE.to_string());
            glfw_backend.window.set_pos(x, y);
            glfw_backend.window.set_size(w, h);
            self.virtual_origin_x = x as f32;
            self.virtual_origin_y = y as f32;
            self.window_positioned = true;
        }

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("ime_indicator_overlay"),
        ));

        let snapshot = *self.shared_state.lock().expect("overlay state poisoned");

        if let Some(caret) = snapshot.caret {
            let caret_pos = self.screen_px_to_overlay_point(
                caret.x as f32,
                caret.y as f32,
                pixels_per_point,
            );
            self.draw_indicator(
                &painter,
                caret_pos.x,
                caret_pos.y,
                caret.height as f32 / pixels_per_point,
                self.caret_font_px,
                self.caret_offset_x / pixels_per_point,
                self.caret_offset_y / pixels_per_point,
                snapshot.indicator_state,
                false,
            );
        }

        if let Some(mouse) = snapshot.mouse {
            let mouse_pos = self.screen_px_to_overlay_point(
                mouse.x as f32,
                mouse.y as f32,
                pixels_per_point,
            );
            self.draw_indicator(
                &painter,
                mouse_pos.x,
                mouse_pos.y,
                0.0,
                self.mouse_font_px,
                self.mouse_offset_x / pixels_per_point,
                self.mouse_offset_y / pixels_per_point,
                snapshot.indicator_state,
                true,
            );
        }

        glfw_backend.set_passthrough(true);
        glfw_backend.window.set_decorated(false);
        glfw_backend.window.set_floating(true);

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

#[cfg(target_os = "windows")]
fn get_virtual_desktop_rect() -> (i32, i32, i32, i32) {
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
fn get_virtual_desktop_rect() -> (i32, i32, i32, i32) {
    (0, 0, 1920, 1080)
}
