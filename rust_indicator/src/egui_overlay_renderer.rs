use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Color32, FontId, Pos2, Rect, Vec2};
use egui_render_three_d::ThreeDBackend as DefaultGfxBackend;
use overlay_shared::egui_support::load_system_fonts;
use overlay_shared::windows_support::{get_monitor_work_rect_for_point, get_virtual_desktop_rect};

use crate::ime_detector::IndicatorState;

const WINDOW_TITLE: &str = "IME-Indicator-egui";
const PADDING_X: f32 = 6.0;
const PADDING_Y: f32 = 2.0;
const CORNER_RADIUS: f32 = 8.0;
const MIN_BUBBLE_WIDTH: f32 = 24.0;
const MIN_BUBBLE_HEIGHT: f32 = 18.0;
const CARET_MIN_ANCHOR_HEIGHT: f32 = 20.0;
const CARET_GAP_Y: f32 = 2.0;
const MOUSE_GAP_X: f32 = 18.0;
const MOUSE_GAP_Y: f32 = 20.0;
const SCREEN_MARGIN: f32 = 6.0;

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
pub struct MouseVisual;

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
    system_status: SystemStatus,
    last_status_poll: Instant,
}

#[derive(Clone, Debug)]
struct SystemStatus {
    cursor_visible: bool,
    input_method: String,
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            cursor_visible: true,
            input_method: "?".to_string(),
        }
    }
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
            system_status: SystemStatus::default(),
            last_status_poll: Instant::now() - Duration::from_secs(1),
        }
    }

    fn screen_px_rect_to_overlay_rect(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        pixels_per_point: f32,
    ) -> Rect {
        Rect::from_min_size(
            Pos2::new(
                (x as f32 - self.virtual_origin_x) / pixels_per_point,
                (y as f32 - self.virtual_origin_y) / pixels_per_point,
            ),
            Vec2::new(w as f32 / pixels_per_point, h as f32 / pixels_per_point),
        )
    }

    fn monitor_overlay_rect_for_point(
        &self,
        screen_x: f32,
        screen_y: f32,
        pixels_per_point: f32,
    ) -> Rect {
        let (x, y, w, h) = get_monitor_work_rect_for_point(screen_x as i32, screen_y as i32);
        self.screen_px_rect_to_overlay_rect(x, y, w, h, pixels_per_point)
    }

    fn top_left_bubble_pos(&self, screen: Rect, anchor: Pos2, size: Vec2) -> Pos2 {
        let mut pos = Pos2::new(
            anchor.x - size.x - MOUSE_GAP_X,
            anchor.y - size.y - MOUSE_GAP_Y,
        );

        if pos.x < screen.left() + SCREEN_MARGIN {
            pos.x = anchor.x + MOUSE_GAP_X;
        }
        if pos.y < screen.top() + SCREEN_MARGIN {
            pos.y = anchor.y + MOUSE_GAP_Y;
        }

        pos.x = pos.x.clamp(
            screen.left() + SCREEN_MARGIN,
            screen.right() - size.x - SCREEN_MARGIN,
        );
        pos.y = pos.y.clamp(
            screen.top() + SCREEN_MARGIN,
            screen.bottom() - size.y - SCREEN_MARGIN,
        );
        pos
    }

    fn bottom_right_bubble_pos(&self, screen: Rect, anchor: Pos2, size: Vec2) -> Pos2 {
        let mut pos = Pos2::new(anchor.x + MOUSE_GAP_X, anchor.y + MOUSE_GAP_Y);

        if pos.x + size.x > screen.right() - SCREEN_MARGIN {
            pos.x = anchor.x - size.x - MOUSE_GAP_X;
        }
        if pos.y + size.y > screen.bottom() - SCREEN_MARGIN {
            pos.y = anchor.y - size.y - MOUSE_GAP_Y;
        }

        pos.x = pos.x.clamp(
            screen.left() + SCREEN_MARGIN,
            screen.right() - size.x - SCREEN_MARGIN,
        );
        pos.y = pos.y.clamp(
            screen.top() + SCREEN_MARGIN,
            screen.bottom() - size.y - SCREEN_MARGIN,
        );
        pos
    }

    fn load_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded {
            return;
        }
        load_system_fonts(ctx);
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
        mouse_screen: Option<Rect>,
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
            let screen = mouse_screen.unwrap_or_else(|| painter.clip_rect());
            self.top_left_bubble_pos(
                screen,
                Pos2::new(anchor_x + offset_x, anchor_y + offset_y),
                Vec2::new(bubble_width, bubble_height),
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

    fn poll_status(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_status_poll) >= Duration::from_millis(50) {
            self.system_status = query_system_status();
            self.last_status_poll = now;
        }
    }

    fn draw_mouse_status(&self, painter: &egui::Painter, mouse: Pos2, screen: Rect) {
        if !self.system_status.cursor_visible {
            return;
        }

        let font = FontId::proportional(14.0);
        let color = Color32::from_rgb(130, 214, 255);
        let galley = painter.layout_no_wrap(self.system_status.input_method.clone(), font, color);
        let padding = Vec2::new(10.0, 6.0);
        let size = Vec2::new(galley.rect.width(), galley.rect.height()) + padding * 2.0;
        let pos = self.bottom_right_bubble_pos(screen, mouse, size);

        let rect = Rect::from_min_size(pos, size);
        painter.rect_filled(rect, CORNER_RADIUS, background_color());
        painter.galley(pos + padding, galley, color);
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

        self.poll_status();

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
                None,
            );
        }

        if snapshot.mouse.is_some() {
            let mouse_pos = Pos2::new(glfw_backend.cursor_pos[0], glfw_backend.cursor_pos[1]);
            let mouse_screen = self.monitor_overlay_rect_for_point(
                glfw_backend.cursor_pos[0] * pixels_per_point + self.virtual_origin_x,
                glfw_backend.cursor_pos[1] * pixels_per_point + self.virtual_origin_y,
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
                Some(mouse_screen),
            );
            self.draw_mouse_status(&painter, mouse_pos, mouse_screen);
        }

        glfw_backend.set_passthrough(true);
        glfw_backend.window.set_decorated(false);
        glfw_backend.window.set_floating(true);

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

#[cfg(target_os = "windows")]
fn query_system_status() -> SystemStatus {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows::Win32::UI::WindowsAndMessaging::{
        CURSOR_SHOWING, CURSORINFO, GetCursorInfo, GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let mut cursor_info = CURSORINFO {
            cbSize: std::mem::size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        let cursor_visible =
            GetCursorInfo(&mut cursor_info).is_ok() && cursor_info.flags == CURSOR_SHOWING;

        let foreground = GetForegroundWindow();
        let thread_id = GetWindowThreadProcessId(foreground, None);
        let layout = GetKeyboardLayout(thread_id);
        let tsf_status = query_tsf_status(foreground);
        let input_info = foreground_input_method_info(
            layout,
            tsf_status
                .as_ref()
                .and_then(|status| status.input_method.as_deref()),
        );

        SystemStatus {
            cursor_visible,
            input_method: input_info.label,
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn query_system_status() -> SystemStatus {
    SystemStatus::default()
}

#[cfg(target_os = "windows")]
struct TsfStatus {
    input_method: Option<String>,
}

#[cfg(target_os = "windows")]
struct InputMethodInfo {
    label: String,
}

#[cfg(target_os = "windows")]
fn query_tsf_status(foreground: windows::Win32::Foundation::HWND) -> Option<TsfStatus> {
    use std::sync::Once;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfInputProcessorProfileMgr,
        ITfInputProcessorProfiles, TF_INPUTPROCESSORPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    use windows::core::IUnknown;

    static COM_INIT: Once = Once::new();

    unsafe {
        COM_INIT.call_once(|| {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        });

        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let input_method = with_foreground_thread_input_attached(foreground_thread, || {
            let profiles: ITfInputProcessorProfiles = CoCreateInstance(
                &CLSID_TF_InputProcessorProfiles,
                None::<&IUnknown>,
                CLSCTX_INPROC_SERVER,
            )
            .ok()?;
            let profile_mgr: ITfInputProcessorProfileMgr = CoCreateInstance(
                &CLSID_TF_InputProcessorProfiles,
                None::<&IUnknown>,
                CLSCTX_INPROC_SERVER,
            )
            .ok()?;

            let mut profile = TF_INPUTPROCESSORPROFILE::default();
            let input_method = profile_mgr
                .GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut profile)
                .ok()
                .and_then(|_| {
                    if profile.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR {
                        let description = profiles
                            .GetLanguageProfileDescription(
                                &profile.clsid,
                                profile.langid,
                                &profile.guidProfile,
                            )
                            .ok()
                            .map(|value| value.to_string());
                        let mapped = map_tsf_profile_label(
                            &format!("{:?}", profile.clsid),
                            &format!("{:?}", profile.guidProfile),
                            description.as_deref(),
                        );
                        Some(mapped.unwrap_or_else(|| {
                            compact_input_method_label(description.as_deref().unwrap_or("?"))
                        }))
                    } else {
                        Some(layout_label_from_hkl(profile.hkl))
                    }
                });

            Some(input_method)
        })
        .unwrap_or(None);

        Some(TsfStatus { input_method })
    }
}

#[cfg(target_os = "windows")]
fn with_foreground_thread_input_attached<T>(foreground_thread: u32, f: impl FnOnce() -> T) -> T {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};

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

#[cfg(target_os = "windows")]
fn map_tsf_profile_label(
    clsid: &str,
    profile_guid: &str,
    description: Option<&str>,
) -> Option<String> {
    let clsid = clsid.to_ascii_uppercase();
    let profile_guid = profile_guid.to_ascii_uppercase();

    let label = match (clsid.as_str(), profile_guid.as_str()) {
        ("{86598FB9-66A2-463E-B9C2-AEB906D477AD}", "{607FDF85-FCC8-4DBD-A365-41296F980C9C}") => {
            Some("MS")
        }
        ("{82B1D863-86B6-8B98-B8FF-A585BD0F186F}", "{82B1D863-86B6-8B98-B8FF-A585BD0F186F}") => {
            Some("QQ")
        }
        ("{A3F4CDED-B1E9-41EE-9CA6-7B4D0DE6CB0A}", "{3D02CAB6-2B8E-4781-BA20-1C9267529467}") => {
            Some("RM")
        }
        _ => None,
    };

    label
        .map(str::to_string)
        .or_else(|| description.map(compact_input_method_label))
}

#[cfg(target_os = "windows")]
fn foreground_input_method_info(
    layout: windows::Win32::UI::Input::KeyboardAndMouse::HKL,
    tsf_label: Option<&str>,
) -> InputMethodInfo {
    let layout_label = imm_input_method_label(layout);
    let preferred_label = prefer_specific_input_label(
        layout_label,
        tsf_label
            .map(|label| label.to_string())
            .unwrap_or_else(|| "?".to_string()),
    );
    if is_english_hkl(layout) || preferred_label == "EN" {
        return InputMethodInfo {
            label: "EN".to_string(),
        };
    }

    InputMethodInfo {
        label: preferred_label,
    }
}

#[cfg(target_os = "windows")]
fn imm_input_method_label(layout: windows::Win32::UI::Input::KeyboardAndMouse::HKL) -> String {
    use windows::Win32::UI::Input::Ime::{ImmGetDescriptionW, ImmGetIMEFileNameW};

    unsafe {
        let file_label = {
            let mut filename = [0u16; 260];
            let len = ImmGetIMEFileNameW(layout, Some(&mut filename));
            if len > 0 {
                compact_input_method_label(&String::from_utf16_lossy(&filename[..len as usize]))
            } else {
                "?".to_string()
            }
        };

        let mut description = [0u16; 128];
        let desc_len = ImmGetDescriptionW(layout, Some(&mut description));
        if desc_len > 0 {
            let description_label = compact_input_method_label(&String::from_utf16_lossy(
                &description[..desc_len as usize],
            ));
            prefer_specific_input_label(description_label, file_label)
        } else {
            prefer_specific_input_label(file_label, layout_label_from_hkl(layout))
        }
    }
}

#[cfg(target_os = "windows")]
fn compact_input_method_label(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.contains("qq") {
        "QQ".to_string()
    } else if lower.contains("rime")
        || lower.contains("weasel")
        || lower.contains("fcitx")
        || lower.contains("squirrel")
        || lower.contains("trime")
        || name.contains("中州韵")
    {
        "RM".to_string()
    } else if lower.contains("wetype")
        || lower.contains("microsoft")
        || lower.contains("chsime")
        || lower.contains("mspy")
        || lower.contains("pinyin")
        || lower.contains("shuangpin")
        || lower.contains("weixin")
        || lower.contains("wechat")
    {
        "MS".to_string()
    } else if name.contains("英语")
        || lower.contains("english")
        || lower == "us"
        || lower.contains("united states")
    {
        "EN".to_string()
    } else if name.contains("中文") || name.contains("中国") || lower.contains("chinese") {
        "ZH".to_string()
    } else if name.contains('日') || lower.contains("japanese") {
        "JP".to_string()
    } else if name.contains('韩') || name.contains('朝') || lower.contains("korean") {
        "KO".to_string()
    } else {
        name.chars().next().unwrap_or('?').to_string()
    }
}

#[cfg(target_os = "windows")]
fn prefer_specific_input_label(primary: String, fallback: String) -> String {
    if is_specific_input_label(&primary) {
        primary
    } else if is_specific_input_label(&fallback) {
        fallback
    } else if primary != "?" {
        primary
    } else {
        fallback
    }
}

#[cfg(target_os = "windows")]
fn is_specific_input_label(label: &str) -> bool {
    matches!(label, "QQ" | "RM" | "MS" | "EN" | "ZH" | "JP" | "KO")
}

#[cfg(target_os = "windows")]
fn is_english_hkl(layout: windows::Win32::UI::Input::KeyboardAndMouse::HKL) -> bool {
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

#[cfg(target_os = "windows")]
fn layout_label_from_hkl(layout: windows::Win32::UI::Input::KeyboardAndMouse::HKL) -> String {
    let lang_id = format!("{:08X}", (layout.0 as usize) & 0xffff);
    layout_label_from_klid(&lang_id)
}

#[cfg(target_os = "windows")]
fn layout_label_from_klid(klid: &str) -> String {
    match klid.to_ascii_uppercase().as_str() {
        "00000409" => "EN".to_string(),
        "00000804" | "00000404" | "00000C04" | "00001004" | "00001404" => "ZH".to_string(),
        "00000411" => "JP".to_string(),
        "00000412" => "KO".to_string(),
        _ => "?".to_string(),
    }
}
