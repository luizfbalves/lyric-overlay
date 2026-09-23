use crate::config::Mode;

pub const BASE_W: f64 = 900.0;
pub const BASE_H: f64 = 130.0;
pub const BASE_H_BOTH: f64 = 170.0;
pub const BOTTOM_MARGIN: f64 = 120.0;
pub const MAX_W_FRACTION: f64 = 0.9;

/// Retângulo em pixels físicos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub fn overlay_size(scale: f64, mode: Mode, monitor: Rect, scale_factor: f64) -> (u32, u32) {
    let base_h = if mode == Mode::Both { BASE_H_BOTH } else { BASE_H };
    let w = (BASE_W * scale * scale_factor).min(monitor.w as f64 * MAX_W_FRACTION);
    let h = base_h * scale * scale_factor;
    (w.round() as u32, h.round() as u32)
}

pub fn default_position(monitor: Rect, size: (u32, u32), scale_factor: f64) -> (i32, i32) {
    let x = monitor.x + (monitor.w as i32 - size.0 as i32) / 2;
    let y = monitor.y + monitor.h as i32 - size.1 as i32 - (BOTTOM_MARGIN * scale_factor).round() as i32;
    (x, y)
}

pub fn keep_center(pos: (i32, i32), old: (u32, u32), new: (u32, u32)) -> (i32, i32) {
    (pos.0 + (old.0 as i32 - new.0 as i32) / 2, pos.1 + (old.1 as i32 - new.1 as i32) / 2)
}

pub fn center_on_any(pos: (i32, i32), size: (u32, u32), monitors: &[Rect]) -> bool {
    let cx = pos.0 + size.0 as i32 / 2;
    let cy = pos.1 + size.1 as i32 / 2;
    monitors
        .iter()
        .any(|m| cx >= m.x && cx < m.x + m.w as i32 && cy >= m.y && cy < m.y + m.h as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: Rect = Rect { x: 0, y: 0, w: 1920, h: 1080 };

    #[test]
    fn sizes() {
        assert_eq!(overlay_size(1.0, Mode::Original, FHD, 1.0), (900, 130));
        assert_eq!(overlay_size(1.0, Mode::Both, FHD, 1.0), (900, 170));
        assert_eq!(overlay_size(0.8, Mode::Translated, FHD, 1.0), (720, 104));
        assert_eq!(overlay_size(1.0, Mode::Original, Rect { w: 3840, h: 2160, ..FHD }, 2.0), (1800, 260));
    }

    #[test]
    fn width_capped_at_90_percent() {
        let small = Rect { x: 0, y: 0, w: 1280, h: 800 };
        assert_eq!(overlay_size(1.5, Mode::Both, small, 1.0), (1152, 255));
    }

    #[test]
    fn default_position_is_bottom_center() {
        assert_eq!(default_position(FHD, (900, 130), 1.0), (510, 830));
        let second = Rect { x: 1920, y: -200, w: 1920, h: 1080 };
        assert_eq!(default_position(second, (900, 130), 1.0), (2430, 630));
        assert_eq!(default_position(Rect { w: 3840, h: 2160, ..FHD }, (1800, 260), 2.0), (1020, 1660));
    }

    #[test]
    fn keep_center_on_resize() {
        assert_eq!(keep_center((510, 830), (900, 130), (1125, 163)), (398, 814));
    }

    #[test]
    fn detects_offscreen() {
        let mons = [FHD, Rect { x: -1280, y: 0, w: 1280, h: 1024 }];
        assert!(center_on_any((510, 830), (900, 130), &mons));
        assert!(center_on_any((-1000, 100), (900, 130), &mons));
        assert!(!center_on_any((5000, 100), (900, 130), &mons));
        assert!(!center_on_any((510, 1100), (900, 130), &mons));
    }
}
