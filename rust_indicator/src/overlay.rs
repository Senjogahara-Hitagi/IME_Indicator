//! GDI+ overlay renderer.

use crate::ime_detector::IndicatorState;
use std::ptr::null_mut;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::Graphics::GdiPlus::{
    GdipAddPathArc, GdipClosePathFigure, GdipCreateFont, GdipCreateFontFamilyFromName,
    GdipCreateFromHDC, GdipCreatePath, GdipCreateSolidFill, GdipDeleteBrush, GdipDeleteFont,
    GdipDeleteFontFamily, GdipDeleteGraphics, GdipDeletePath, GdipDrawString, GdipFillPath,
    GdipMeasureString, GdipSetSmoothingMode, GdipSetTextRenderingHint, GdiplusShutdown,
    GdiplusStartup, GdiplusStartupInput, FillMode, GpBrush, GpFont, GpFontFamily, GpPath,
    RectF, SmoothingModeAntiAlias, TextRenderingHintAntiAlias, UnitPixel,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
    RegisterClassExW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    HWND_TOPMOST, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOW,
    ULW_ALPHA, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

#[repr(C)]
struct BLENDFUNCTION {
    blend_op: u8,
    blend_flags: u8,
    source_constant_alpha: u8,
    alpha_format: u8,
}

const AC_SRC_OVER: u8 = 0x00;
const AC_SRC_ALPHA: u8 = 0x01;
const BACKGROUND_COLOR: u32 = 0xAA0C1216;
const TEXT_COLOR_CN: u32 = 0xFF82D6FF;
const TEXT_COLOR_EN: u32 = 0xFFF4C060;
const PADDING_X: f32 = 6.0;
const PADDING_Y: f32 = 2.0;
const CORNER_RADIUS: f32 = 8.0;
const MIN_BUBBLE_WIDTH: f32 = 24.0;
const MIN_BUBBLE_HEIGHT: f32 = 18.0;

pub struct IndicatorOverlay {
    hwnd: HWND,
    canvas_size: i32,
    offset_x: i32,
    offset_y: i32,
    gdi_token: usize,
    font_family: *mut GpFontFamily,
    font: *mut GpFont,
    is_mouse_overlay: bool,
}

impl IndicatorOverlay {
    pub fn new(
        name: &str,
        size: i32,
        _color_cn: u32,
        _color_en: u32,
        offset_x: i32,
        offset_y: i32,
    ) -> Self {
        let gdi_token = Self::init_gdiplus();
        let font_px = (size as f32 + 3.0).max(18.0);
        let canvas_size = (size as f32 * 3.0).max(48.0) as i32;
        let hwnd = Self::create_window(name, canvas_size);

        let mut font_family = null_mut();
        let mut font = null_mut();
        unsafe {
            let font_name = HSTRING::from("Microsoft YaHei");
            let _ = GdipCreateFontFamilyFromName(
                PCWSTR(font_name.as_ptr()),
                null_mut(),
                &mut font_family,
            );
            let _ = GdipCreateFont(font_family, font_px, 1, UnitPixel, &mut font);
        }

        Self {
            hwnd,
            canvas_size,
            offset_x,
            offset_y,
            gdi_token,
            font_family,
            font,
            is_mouse_overlay: name.eq_ignore_ascii_case("Mouse"),
        }
    }

    fn init_gdiplus() -> usize {
        unsafe {
            let input = GdiplusStartupInput {
                GdiplusVersion: 1,
                DebugEventCallback: 0,
                SuppressBackgroundThread: false.into(),
                SuppressExternalCodecs: false.into(),
            };
            let mut token: usize = 0;
            let _ = GdiplusStartup(&mut token, &input, null_mut());
            token
        }
    }

    fn create_window(name: &str, size: i32) -> HWND {
        unsafe {
            let h_instance: HMODULE = GetModuleHandleW(None).unwrap_or_default();
            let class_name: Vec<u16> = format!("IMEIndicator_{}\0", name).encode_utf16().collect();
            let window_name: Vec<u16> = format!("Indicator_{}\0", name).encode_utf16().collect();

            extern "system" fn wnd_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                unsafe {
                    if msg == 0x0002 {
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: std::mem::transmute(h_instance),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };

            RegisterClassExW(&wc);

            let ex_style = WS_EX_LAYERED
                | WS_EX_TRANSPARENT
                | WS_EX_TOPMOST
                | WS_EX_NOACTIVATE
                | WS_EX_TOOLWINDOW;

            CreateWindowExW(
                ex_style,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(window_name.as_ptr()),
                WS_POPUP,
                0,
                0,
                size,
                size,
                None,
                None,
                h_instance,
                None,
            )
            .unwrap_or_default()
        }
    }

    pub fn update(&self, x: i32, y: i32, state: IndicatorState, caret_h: i32) {
        let text_color = if state.is_chinese() {
            TEXT_COLOR_CN
        } else {
            TEXT_COLOR_EN
        };
        let text = state.get_text();
        let wide_text: Vec<u16> = text.encode_utf16().collect();

        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.canvas_size,
                    biHeight: self.canvas_size,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut ppv_bits: *mut std::ffi::c_void = null_mut();
            let h_bitmap =
                CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut ppv_bits, None, 0)
                    .unwrap_or_default();
            let old_bitmap = SelectObject(mem_dc, h_bitmap);

            let mut graphics = null_mut();
            GdipCreateFromHDC(mem_dc, &mut graphics);
            GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
            GdipSetTextRenderingHint(graphics, TextRenderingHintAntiAlias);

            let mut format = null_mut();
            let _ = windows::Win32::Graphics::GdiPlus::GdipCreateStringFormat(0, 0, &mut format);
            let _ = windows::Win32::Graphics::GdiPlus::GdipSetStringFormatAlign(
                format,
                windows::Win32::Graphics::GdiPlus::StringAlignmentCenter,
            );
            let _ = windows::Win32::Graphics::GdiPlus::GdipSetStringFormatLineAlign(
                format,
                windows::Win32::Graphics::GdiPlus::StringAlignmentCenter,
            );

            let layout_rect = RectF {
                X: 0.0,
                Y: 0.0,
                Width: self.canvas_size as f32,
                Height: self.canvas_size as f32,
            };
            let mut bounds = RectF::default();
            let _ = GdipMeasureString(
                graphics,
                PCWSTR(wide_text.as_ptr()),
                wide_text.len() as i32,
                self.font,
                &layout_rect,
                format,
                &mut bounds,
                null_mut(),
                null_mut(),
            );

            let bubble_width = (bounds.Width + PADDING_X * 2.0).max(MIN_BUBBLE_WIDTH);
            let bubble_height = (bounds.Height + PADDING_Y * 2.0).max(MIN_BUBBLE_HEIGHT);
            let bubble_x = ((self.canvas_size as f32) - bubble_width) / 2.0;
            let bubble_y = ((self.canvas_size as f32) - bubble_height) / 2.0;
            let bubble_rect = RectF {
                X: bubble_x,
                Y: bubble_y,
                Width: bubble_width,
                Height: bubble_height,
            };

            let mut background_brush = null_mut();
            GdipCreateSolidFill(BACKGROUND_COLOR, &mut background_brush);
            let mut path = null_mut();
            let _ = GdipCreatePath(FillMode(0), &mut path);
            add_rounded_rect(path, &bubble_rect, CORNER_RADIUS);
            let _ = GdipFillPath(graphics, background_brush as *mut GpBrush, path);

            let mut text_brush = null_mut();
            GdipCreateSolidFill(text_color, &mut text_brush);
            let _ = GdipDrawString(
                graphics,
                PCWSTR(wide_text.as_ptr()),
                wide_text.len() as i32,
                self.font,
                &bubble_rect,
                format,
                text_brush as *mut GpBrush,
            );

            let _ = windows::Win32::Graphics::GdiPlus::GdipDeleteStringFormat(format);
            let _ = GdipDeletePath(path);
            let _ = GdipDeleteBrush(background_brush as *mut GpBrush);
            let _ = GdipDeleteBrush(text_brush as *mut GpBrush);
            let _ = GdipDeleteGraphics(graphics);

            let dest_point = if self.is_mouse_overlay {
                POINT {
                    x: x + self.offset_x - self.canvas_size,
                    y: y + self.offset_y - self.canvas_size / 2,
                }
            } else {
                POINT {
                    x: x + self.offset_x - self.canvas_size / 2,
                    y: y + caret_h + self.offset_y - self.canvas_size / 2,
                }
            };

            let src_point = POINT { x: 0, y: 0 };
            let size = SIZE {
                cx: self.canvas_size,
                cy: self.canvas_size,
            };
            let blend = BLENDFUNCTION {
                blend_op: AC_SRC_OVER,
                blend_flags: 0,
                source_constant_alpha: 255,
                alpha_format: AC_SRC_ALPHA,
            };

            let _ = UpdateLayeredWindow(
                self.hwnd,
                screen_dc,
                Some(&dest_point),
                Some(&size),
                mem_dc,
                Some(&src_point),
                COLORREF(0),
                Some(&blend as *const BLENDFUNCTION as *const _),
                ULW_ALPHA,
            );

            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(h_bitmap);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, screen_dc);

            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );

            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn cleanup(&self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            let _ = GdipDeleteFont(self.font);
            let _ = GdipDeleteFontFamily(self.font_family);
            GdiplusShutdown(self.gdi_token);
        }
    }
}

fn add_rounded_rect(path: *mut GpPath, rect: &RectF, radius: f32) {
    unsafe {
        let diameter = (radius * 2.0).min(rect.Width).min(rect.Height);
        let right = rect.X + rect.Width - diameter;
        let bottom = rect.Y + rect.Height - diameter;

        let _ = GdipAddPathArc(path, rect.X, rect.Y, diameter, diameter, 180.0, 90.0);
        let _ = GdipAddPathArc(path, right, rect.Y, diameter, diameter, 270.0, 90.0);
        let _ = GdipAddPathArc(path, right, bottom, diameter, diameter, 0.0, 90.0);
        let _ = GdipAddPathArc(path, rect.X, bottom, diameter, diameter, 90.0, 90.0);
        let _ = GdipClosePathFigure(path);
    }
}

impl Drop for IndicatorOverlay {
    fn drop(&mut self) {
        self.cleanup();
    }
}
