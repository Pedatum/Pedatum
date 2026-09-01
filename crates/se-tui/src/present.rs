//! A frame of pixels as terminal cells.
//!
//! Every cell is two stacked pixels: `▀` with the top pixel as foreground and
//! the bottom as background. A terminal is therefore a framebuffer of
//! `cols × rows*2`, which is the whole reason the render graph can present
//! here without knowing it.

/// One frame of tightly packed 8-bit RGBA.
pub struct Pixels<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

impl Pixels<'_> {
    /// Clamped sample, so a panel that is briefly out of step with the
    /// framebuffer shows an edge pixel rather than panicking.
    #[inline]
    pub fn rgb(&self, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y.min(self.height.saturating_sub(1)) * self.width + x.min(self.width.saturating_sub(1))) * 4) as usize;
        match self.rgba.get(i..i + 3) {
            Some(p) => (p[0], p[1], p[2]),
            None => (0, 0, 0),
        }
    }
}

/// Terminal cell size for a viewport, in pixels. One cell is two stacked
/// pixels, so a panel `w` by `h` cells is a framebuffer `w` by `h*2`.
pub fn pixel_size(cols: u16, rows: u16) -> (u32, u32) {
    (cols.max(1) as u32, (rows.max(1) as u32) * 2)
}
