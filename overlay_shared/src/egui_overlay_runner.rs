use std::time::Duration;

use egui::{Context, PlatformOutput};
use egui_overlay::egui_render_three_d::ThreeDBackend as DefaultGfxBackend;
use egui_overlay::egui_window_glfw_passthrough;
use egui_overlay::egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};

use crate::windows_support::apply_overlay_window_style;

pub fn start_overlay<T: egui_overlay::EguiOverlay + 'static>(user_data: T) {
    let mut glfw_backend = GlfwBackend::new(GlfwConfig {
        glfw_callback: Box::new(|gtx| {
            (egui_window_glfw_passthrough::GlfwConfig::default().glfw_callback)(gtx);
            gtx.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::ScaleToMonitor(true));
            gtx.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::Visible(false));
            gtx.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::Focused(false));
            gtx.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::FocusOnShow(false));
            gtx.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::MousePassthrough(true));
        }),
        window_callback: Box::new(|window| {
            window.set_floating(true);
            window.set_decorated(false);
            window.set_mouse_passthrough(true);
        }),
        opengl_window: Some(true),
        transparent_window: Some(true),
        ..Default::default()
    });

    #[cfg(target_os = "windows")]
    {
        let hwnd = windows::Win32::Foundation::HWND(glfw_backend.window.get_win32_window());
        apply_overlay_window_style(hwnd);
    }

    glfw_backend.set_passthrough(true);
    glfw_backend.window.show();

    let latest_size = glfw_backend.window.get_framebuffer_size();
    let latest_size = [latest_size.0 as _, latest_size.1 as _];
    let default_gfx_backend = DefaultGfxBackend::new(
        egui_overlay::egui_render_three_d::ThreeDConfig::default(),
        |s| glfw_backend.get_proc_address(s),
        latest_size,
    );

    let overlay_app = OverlayApp {
        user_data,
        egui_context: Default::default(),
        default_gfx_backend,
        glfw_backend,
    };
    overlay_app.enter_event_loop();
}

struct OverlayApp<T: egui_overlay::EguiOverlay + 'static> {
    user_data: T,
    egui_context: Context,
    default_gfx_backend: DefaultGfxBackend,
    glfw_backend: GlfwBackend,
}

impl<T: egui_overlay::EguiOverlay + 'static> OverlayApp<T> {
    fn enter_event_loop(mut self) {
        let mut wait_events_duration = Duration::ZERO;
        loop {
            self.glfw_backend
                .glfw
                .wait_events_timeout(wait_events_duration.as_secs_f64());

            self.glfw_backend.tick();

            if self.glfw_backend.resized_event_pending {
                let latest_size = self.glfw_backend.window.get_framebuffer_size();
                self.default_gfx_backend
                    .resize_framebuffer([latest_size.0 as _, latest_size.1 as _]);
                self.glfw_backend.resized_event_pending = false;
            }

            if let Some((platform_output, timeout)) = run_overlay_frame(
                &mut self.user_data,
                &self.egui_context,
                &mut self.default_gfx_backend,
                &mut self.glfw_backend,
            ) {
                wait_events_duration = timeout.min(Duration::from_secs(1));
                if !platform_output.copied_text.is_empty() {
                    self.glfw_backend
                        .window
                        .set_clipboard_string(&platform_output.copied_text);
                }
                self.glfw_backend.set_cursor(platform_output.cursor_icon);
            } else {
                wait_events_duration = Duration::ZERO;
            }

            if self.glfw_backend.window.should_close() {
                break;
            }
        }
    }
}

fn run_overlay_frame<T: egui_overlay::EguiOverlay>(
    user_data: &mut T,
    egui_context: &Context,
    default_gfx_backend: &mut DefaultGfxBackend,
    glfw_backend: &mut GlfwBackend,
) -> Option<(PlatformOutput, Duration)> {
    let input = glfw_backend.take_raw_input();
    default_gfx_backend.prepare_frame(|| {
        let latest_size = glfw_backend.window.get_framebuffer_size();
        [latest_size.0 as _, latest_size.1 as _]
    });
    egui_context.begin_pass(input);
    user_data.gui_run(egui_context, default_gfx_backend, glfw_backend);

    let egui::FullOutput {
        platform_output,
        textures_delta,
        shapes,
        pixels_per_point,
        viewport_output,
    } = egui_context.end_pass();
    let meshes = egui_context.tessellate(shapes, pixels_per_point);
    let repaint_after = viewport_output
        .into_iter()
        .map(|f| f.1.repaint_delay)
        .collect::<Vec<Duration>>()[0];

    default_gfx_backend.render_egui(meshes, textures_delta, glfw_backend.window_size_logical);
    use egui_window_glfw_passthrough::glfw::Context as _;
    glfw_backend.window.swap_buffers();
    Some((platform_output, repaint_after))
}
