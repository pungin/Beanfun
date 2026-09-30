//! Locate the MapleStory TW login dialog's **account field** from a
//! screenshot of the game's client area (issue #395).
//!
//! # Why this exists
//!
//! The special-click branch of [`super::auto_paste`] used to click at a
//! fixed fraction of the client area — `(0.5 × width, 0.4 × height)`,
//! inherited from WPF `MainWindow.xaml.cs` L2206-2207. That only holds
//! for the layout the ratio was tuned on. With the game's "擴展UI模式"
//! (extended UI mode) the client remembers a larger window on the next
//! launch and the login panel is drawn either uniformly scaled (a
//! 2560 × 1440 client shows the panel at ~2×) or at native size, so
//! the fixed fraction may land on the panel background instead of the
//! account box.
//!
//! A missed click is not harmless: a `WM_LBUTTONDOWN` on the background
//! leaves keyboard focus where it was. After a logout that is the
//! **password** field, so the account goes into the password box, the
//! following `TAB` moves to the account box, and the OTP is typed there
//! — exactly the symptom in #395 ("the OTP ends up in the account
//! field"). The first login of a session works only because focus
//! starts on the account box.
//!
//! # How it works
//!
//! The dialog's 「登入」 button is a solid, pure-cyan (`#00FFFF`)
//! rounded rectangle sitting on a white panel — the most distinctive
//! feature on the screen and the one whose geometry scales with the
//! panel. [`locate_account_field`] finds it as the largest cyan
//! connected component that looks like a wide solid rectangle with
//! white panel on both sides, then derives the account box from it:
//! the account text sits `0.29 × button_width` above the button's top
//! edge, horizontally centred on it. Measured on a 2560 × 1440 client
//! (button 521 × 89 px, account text 151 px above it → 0.290) and on
//! the reporter's 1366 × 768 recording (≈ 0.30).
//!
//! When the screen cannot be captured (exclusive fullscreen, locked
//! desktop) or no button is found, the caller falls back to the WPF
//! ratio so behaviour is never worse than before.

use super::post_string::{Point, Size};

/// A captured client-area frame in top-down BGRA order (4 bytes per
/// pixel, `width * height * 4` bytes), as `GetDIBits` produces for a
/// 32-bpp DIB with a negative height.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub size: Size,
    pub bgra: Vec<u8>,
}

impl Frame {
    /// `(b, g, r)` of the pixel at `(x, y)`; `None` when out of range.
    fn bgr(&self, x: i32, y: i32) -> Option<(u8, u8, u8)> {
        if x < 0 || y < 0 || x >= self.size.width || y >= self.size.height {
            return None;
        }
        let i = ((y * self.size.width + x) * 4) as usize;
        let p = self.bgra.get(i..i + 3)?;
        Some((p[0], p[1], p[2]))
    }

    fn is_cyan(&self, x: i32, y: i32) -> bool {
        matches!(self.bgr(x, y), Some((b, g, r)) if r <= CYAN_MAX_RED && g >= CYAN_MIN_GB && b >= CYAN_MIN_GB)
    }

    fn is_white(&self, x: i32, y: i32) -> bool {
        matches!(self.bgr(x, y), Some((b, g, r)) if r >= WHITE_MIN && g >= WHITE_MIN && b >= WHITE_MIN)
    }
}

/// Button pixels are `#00FFFF`; allow for colour management / capture
/// rounding but stay clear of the sky gradient, which is lighter and
/// never bordered by the white panel.
const CYAN_MAX_RED: u8 = 60;
const CYAN_MIN_GB: u8 = 220;
/// The login panel is `#FFFFFF`.
const WHITE_MIN: u8 = 235;

/// Button width as a fraction of the client width — 521 / 2560 = 0.20
/// on the scaled layout, ≈ 272 / 1366 = 0.20 on the native one.
const MIN_BUTTON_WIDTH_RATIO: f64 = 0.08;
const MAX_BUTTON_WIDTH_RATIO: f64 = 0.45;
/// Button aspect ratio (width / height): 521 / 89 = 5.85.
const MIN_ASPECT: f64 = 3.5;
const MAX_ASPECT: f64 = 8.5;
/// Fraction of the bounding box covered by cyan — the 「登入」 glyphs
/// are white so a real button lands around 0.97.
const MIN_FILL: f64 = 0.85;
/// Minimum button height in pixels; anything thinner is a line.
const MIN_BUTTON_HEIGHT: i32 = 12;
/// Account-text centre sits this many button-widths above the button's
/// top edge (see module docs for the measurements).
const ACCOUNT_OFFSET_RATIO: f64 = 0.29;
/// Distance outside the button's bounding box where the white panel is
/// probed.
const PANEL_PROBE_MARGIN: i32 = 6;

/// Bounding box of the located 「登入」 button, client coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl ButtonRect {
    fn centre_x(&self) -> i32 {
        self.left + self.width / 2
    }
}

/// Where to click for the account field, derived from `button`.
pub fn account_point_from_button(button: ButtonRect) -> Point {
    Point {
        x: button.centre_x(),
        y: button.top - (button.width as f64 * ACCOUNT_OFFSET_RATIO).round() as i32,
    }
}

/// Find the 「登入」 button in `frame`. Returns `None` when nothing
/// passes the shape and surroundings checks.
pub fn locate_login_button(frame: &Frame) -> Option<ButtonRect> {
    let w = frame.size.width;
    let h = frame.size.height;
    if w <= 0 || h <= 0 || frame.bgra.len() < (w as usize) * (h as usize) * 4 {
        return None;
    }

    let min_w = (w as f64 * MIN_BUTTON_WIDTH_RATIO) as i32;
    let max_w = (w as f64 * MAX_BUTTON_WIDTH_RATIO) as i32;

    let mut visited = vec![false; (w as usize) * (h as usize)];
    let mut stack: Vec<(i32, i32)> = Vec::new();
    let mut best: Option<(i64, ButtonRect)> = None;

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if visited[idx] || !frame.is_cyan(x, y) {
                continue;
            }
            // Flood-fill this cyan component, tracking area + bbox.
            visited[idx] = true;
            stack.push((x, y));
            let (mut area, mut min_x, mut max_x, mut min_y, mut max_y) = (0i64, x, x, y, y);
            while let Some((cx, cy)) = stack.pop() {
                area += 1;
                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);
                for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let nidx = (ny * w + nx) as usize;
                    if !visited[nidx] && frame.is_cyan(nx, ny) {
                        visited[nidx] = true;
                        stack.push((nx, ny));
                    }
                }
            }

            let rect = ButtonRect {
                left: min_x,
                top: min_y,
                width: max_x - min_x + 1,
                height: max_y - min_y + 1,
            };
            if !looks_like_login_button(frame, rect, area, min_w, max_w) {
                continue;
            }
            if best.map_or(true, |(best_area, _)| area > best_area) {
                best = Some((area, rect));
            }
        }
    }

    best.map(|(_, rect)| rect)
}

fn looks_like_login_button(
    frame: &Frame,
    rect: ButtonRect,
    area: i64,
    min_w: i32,
    max_w: i32,
) -> bool {
    if rect.height < MIN_BUTTON_HEIGHT || rect.width < min_w || rect.width > max_w {
        return false;
    }
    let aspect = rect.width as f64 / rect.height as f64;
    if !(MIN_ASPECT..=MAX_ASPECT).contains(&aspect) {
        return false;
    }
    let fill = area as f64 / (rect.width as f64 * rect.height as f64);
    if fill < MIN_FILL {
        return false;
    }
    // The button sits on the white panel: probe just outside both
    // vertical edges at mid-height …
    let mid_y = rect.top + rect.height / 2;
    if !frame.is_white(rect.left - PANEL_PROBE_MARGIN, mid_y)
        || !frame.is_white(rect.left + rect.width - 1 + PANEL_PROBE_MARGIN, mid_y)
    {
        return false;
    }
    // … and the account row above it must also be panel (white). The
    // account text is left-aligned, so probe the right-hand part of
    // the row where no text can be.
    let account = account_point_from_button(rect);
    account.y > 0
        && frame.is_white(rect.left + (rect.width as f64 * 0.8) as i32, account.y)
        && frame.is_white(rect.left + (rect.width as f64 * 0.9) as i32, account.y)
}

/// Locate the account field: button detection + offset. `None` means
/// "fall back to the ratio click".
pub fn locate_account_field(frame: &Frame) -> Option<Point> {
    locate_login_button(frame).map(account_point_from_button)
}

/// Capture `size` pixels of the screen starting at `origin` (screen
/// coordinates, physical pixels) into a top-down BGRA [`Frame`].
///
/// Uses GDI `BitBlt` from the screen DC, which works for a windowed
/// Direct3D client (the WebView-free path the auto-paste sequence
/// already relies on). Returns `None` on any GDI failure, an empty
/// size, or a frame that is entirely black (exclusive fullscreen).
#[cfg(target_os = "windows")]
pub fn capture_screen_region(origin: Point, size: Size) -> Option<Frame> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };

    if size.width <= 0 || size.height <= 0 {
        return None;
    }

    // SAFETY: plain GDI calls with handles we create and release in
    // this function; every handle is checked before use and released
    // on all paths below.
    unsafe {
        let screen = GetDC(HWND::default());
        if screen.is_invalid() {
            return None;
        }
        let mem = CreateCompatibleDC(screen);
        if mem.is_invalid() {
            let _ = ReleaseDC(HWND::default(), screen);
            return None;
        }
        let bitmap = CreateCompatibleBitmap(screen, size.width, size.height);
        let result = if bitmap.is_invalid() {
            None
        } else {
            let old = SelectObject(mem, bitmap);
            let blit = BitBlt(
                mem,
                0,
                0,
                size.width,
                size.height,
                screen,
                origin.x,
                origin.y,
                SRCCOPY,
            );
            let frame = if blit.is_ok() {
                let mut info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: size.width,
                        // Negative height → top-down rows.
                        biHeight: -size.height,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bgra = vec![0u8; (size.width as usize) * (size.height as usize) * 4];
                let lines = GetDIBits(
                    mem,
                    bitmap,
                    0,
                    size.height as u32,
                    Some(bgra.as_mut_ptr().cast()),
                    &mut info,
                    DIB_RGB_COLORS,
                );
                if lines == size.height && bgra.iter().any(|&b| b != 0) {
                    Some(Frame { size, bgra })
                } else {
                    None
                }
            } else {
                None
            };
            SelectObject(mem, old);
            let _ = DeleteObject(bitmap);
            frame
        };
        let _ = DeleteDC(mem);
        let _ = ReleaseDC(HWND::default(), screen);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a frame: `bg` everywhere, a white panel rectangle, and a
    /// cyan button rectangle inside it. Colours are `(r, g, b)`.
    fn synth(
        size: Size,
        bg: (u8, u8, u8),
        panel: Option<ButtonRect>,
        button: Option<ButtonRect>,
    ) -> Frame {
        let mut bgra = vec![0u8; (size.width * size.height * 4) as usize];
        let mut put = |x: i32, y: i32, (r, g, b): (u8, u8, u8)| {
            let i = ((y * size.width + x) * 4) as usize;
            bgra[i] = b;
            bgra[i + 1] = g;
            bgra[i + 2] = r;
            bgra[i + 3] = 255;
        };
        for y in 0..size.height {
            for x in 0..size.width {
                put(x, y, bg);
            }
        }
        for (rect, colour) in [(panel, (255, 255, 255)), (button, (0, 255, 255))] {
            if let Some(r) = rect {
                for y in r.top..r.top + r.height {
                    for x in r.left..r.left + r.width {
                        put(x, y, colour);
                    }
                }
            }
        }
        Frame { size, bgra }
    }

    const SIZE: Size = Size {
        width: 640,
        height: 360,
    };
    const PANEL: ButtonRect = ButtonRect {
        left: 200,
        top: 100,
        width: 240,
        height: 160,
    };
    const BUTTON: ButtonRect = ButtonRect {
        left: 220,
        top: 200,
        width: 200,
        height: 34,
    };

    #[test]
    fn locates_button_on_white_panel_and_derives_account_point() {
        let frame = synth(SIZE, (120, 200, 90), Some(PANEL), Some(BUTTON));
        assert_eq!(locate_login_button(&frame), Some(BUTTON));
        // 0.29 × 200 = 58 above the top edge, centred on the button.
        assert_eq!(
            locate_account_field(&frame),
            Some(Point {
                x: 220 + 100,
                y: 200 - 58
            })
        );
    }

    #[test]
    fn measured_2560x1440_button_maps_to_account_row() {
        // Numbers from the live capture that drove this module:
        // button x[1019,1539] y[733,821], account text centre y=582.
        let button = ButtonRect {
            left: 1019,
            top: 733,
            width: 521,
            height: 89,
        };
        let p = account_point_from_button(button);
        assert_eq!(p.x, 1279);
        assert!((p.y - 582).abs() <= 4, "got y={}", p.y);
    }

    #[test]
    fn ignores_cyan_that_is_not_on_a_white_panel() {
        // Same button, but no panel — the sky-like cyan blob has no
        // white surroundings, so it must not be accepted.
        let frame = synth(SIZE, (120, 200, 90), None, Some(BUTTON));
        assert_eq!(locate_login_button(&frame), None);
    }

    #[test]
    fn ignores_cyan_sky_band_that_is_too_wide() {
        let sky = ButtonRect {
            left: 0,
            top: 0,
            width: 640,
            height: 80,
        };
        let frame = synth(SIZE, (255, 255, 255), None, Some(sky));
        assert_eq!(locate_login_button(&frame), None);
    }

    #[test]
    fn ignores_thin_cyan_rule_lines() {
        let rule = ButtonRect {
            left: 220,
            top: 120,
            width: 200,
            height: 3,
        };
        let frame = synth(SIZE, (120, 200, 90), Some(PANEL), Some(rule));
        assert_eq!(locate_login_button(&frame), None);
    }

    #[test]
    fn rejects_frames_with_short_buffers() {
        let frame = Frame {
            size: SIZE,
            bgra: vec![0; 16],
        };
        assert_eq!(locate_login_button(&frame), None);
    }
}
