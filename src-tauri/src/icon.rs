/// 22 pt @2x para a barra de menus.
pub const ICON_PX: u32 = 44;

/// Duas colcheias ligadas, em preto com alfa (ícone "template" do macOS).
pub fn note_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let fx = (x as f32 + 0.5) / s;
            let fy = (y as f32 + 0.5) / s;
            let head = |cx: f32, cy: f32| {
                let dx = (fx - cx) / 0.16;
                let dy = (fy - cy) / 0.12;
                dx * dx + dy * dy <= 1.0
            };
            let beam_top = 0.18 - (fx - 0.40) * (0.08 / 0.50);
            let on = head(0.28, 0.78)
                || head(0.72, 0.70)
                || ((0.40..=0.46).contains(&fx) && (0.18..=0.78).contains(&fy))
                || ((0.84..=0.90).contains(&fx) && (0.10..=0.70).contains(&fy))
                || ((0.40..=0.90).contains(&fx) && fy >= beam_top && fy <= beam_top + 0.12);
            if on {
                px[((y * size + x) * 4 + 3) as usize] = 255;
            }
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_shape_and_transparent_corners() {
        let px = note_rgba(ICON_PX);
        assert_eq!(px.len(), (ICON_PX * ICON_PX * 4) as usize);
        let alpha = |x: u32, y: u32| px[((y * ICON_PX + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(ICON_PX - 1, ICON_PX - 1), 0);
        let opaque = px.chunks(4).filter(|p| p[3] == 255).count();
        assert!(opaque > 100 && opaque < (ICON_PX * ICON_PX / 2) as usize, "opaque = {opaque}");
    }
}
