/// 22 pt @2x para a barra de menus.
pub const ICON_PX: u32 = 44;

/// Disco de vinil com uma linha de letra no selo, em preto com alfa (ícone "template" do macOS).
pub fn disc_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let fx = (x as f32 + 0.5) / s - 0.5;
            let fy = (y as f32 + 0.5) / s - 0.5;
            let r = (fx * fx + fy * fy).sqrt();
            let disc = r <= 0.46 && r > 0.22;
            let line = fx.abs() <= 0.13 && fy.abs() <= 0.04;
            if disc || line {
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
        let px = disc_rgba(ICON_PX);
        assert_eq!(px.len(), (ICON_PX * ICON_PX * 4) as usize);
        let alpha = |x: u32, y: u32| px[((y * ICON_PX + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(ICON_PX - 1, ICON_PX - 1), 0);
        let opaque = px.chunks(4).filter(|p| p[3] == 255).count();
        assert!(opaque > 100 && opaque < (ICON_PX * ICON_PX * 3 / 4) as usize, "opaque = {opaque}");
        let c = ICON_PX / 2;
        assert_eq!(alpha(c, c), 255, "linha no centro do selo");
        assert_eq!(alpha(c, c - ICON_PX / 6), 0, "selo vazado acima da linha");
    }
}
