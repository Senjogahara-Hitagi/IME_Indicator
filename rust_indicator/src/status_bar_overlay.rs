use std::time::{Duration, Instant};

use egui::{Color32, FontId, Pos2, Rect, Vec2};
use egui_render_three_d::ThreeDBackend as DefaultGfxBackend;
use overlay_shared::egui_support::load_system_fonts;
use overlay_shared::windows_support::get_virtual_desktop_rect;

const WINDOW_TITLE: &str = "overlay-status_bar";

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

pub struct StatusOverlayApp {
    fonts_loaded: bool,
    window_positioned: bool,
    system_status: SystemStatus,
    last_status_poll: Instant,
}

impl StatusOverlayApp {
    pub fn new() -> Self {
        Self {
            fonts_loaded: false,
            window_positioned: false,
            system_status: SystemStatus::default(),
            last_status_poll: Instant::now() - Duration::from_secs(1),
        }
    }

    fn load_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded {
            return;
        }
        load_system_fonts(ctx);
        self.fonts_loaded = true;
    }

    fn poll_state(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_status_poll) >= Duration::from_millis(50) {
            self.system_status = query_system_status();
            self.last_status_poll = now;
        }
    }

    fn draw_status(&self, ctx: &egui::Context, mouse: Pos2) {
        if !self.system_status.cursor_visible {
            return;
        }

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("status_overlay"),
        ));
        let font = FontId::proportional(14.0);
        let galley = painter.layout_no_wrap(
            self.system_status.input_method.clone(),
            font,
            Color32::from_rgb(130, 214, 255),
        );
        let padding = Vec2::new(10.0, 6.0);
        let size = Vec2::new(galley.rect.width(), galley.rect.height()) + padding * 2.0;
        let mut pos = mouse + Vec2::new(30.0, 18.0);
        let screen = ctx.screen_rect();

        if pos.x + size.x > screen.right() - 6.0 {
            pos.x = mouse.x - size.x - 30.0;
        }
        if pos.y + size.y > screen.bottom() - 6.0 {
            pos.y = mouse.y - size.y - 18.0;
        }
        pos.x = pos.x.max(screen.left() + 6.0);
        pos.y = pos.y.max(screen.top() + 6.0);

        let rect = Rect::from_min_size(pos, size);
        painter.rect_filled(rect, 8.0, Color32::from_rgba_unmultiplied(12, 18, 22, 170));
        painter.galley(pos + padding, galley, Color32::from_rgb(130, 214, 255));
    }
}

impl egui_overlay::EguiOverlay for StatusOverlayApp {
    fn gui_run(
        &mut self,
        ctx: &egui::Context,
        _default_gfx_backend: &mut DefaultGfxBackend,
        glfw_backend: &mut egui_window_glfw_passthrough::GlfwBackend,
    ) {
        self.load_fonts(ctx);

        if !self.window_positioned {
            let (x, y, w, h) = get_virtual_desktop_rect();
            glfw_backend.set_title(WINDOW_TITLE.to_string());
            glfw_backend.window.set_pos(x, y);
            glfw_backend.window.set_size(w, h);
            self.window_positioned = true;
        }

        self.poll_state();

        let mouse = Pos2::new(glfw_backend.cursor_pos[0], glfw_backend.cursor_pos[1]);
        self.draw_status(ctx, mouse);

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
