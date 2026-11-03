//! Pixel layout for the native-endian, premultiplied wl_shm ARGB8888 format.

#[derive(Debug)]
pub(super) struct IconLayout {
    pub edge: i32,
    pub stride: i32,
}

pub(super) fn icon_layout(width: u32, height: u32) -> Option<IconLayout> {
    let edge = i32::try_from(width.max(height)).ok()?;
    let stride = edge.checked_mul(4)?;
    // wl_shm pool sizes and offsets are signed 32-bit integers.
    stride.checked_mul(edge).filter(|size| *size > 0)?;
    if width == 0 || height == 0 {
        return None;
    }
    Some(IconLayout { edge, stride })
}

pub(super) fn write_argb8888(rgba: &[u8], width: usize, height: usize, canvas: &mut [u8]) {
    let edge = width.max(height);
    let left = (edge - width) / 2;
    let top = (edge - height) / 2;
    canvas.fill(0);
    for (source, row) in
        rgba.chunks_exact(width * 4).zip(canvas.chunks_exact_mut(edge * 4).skip(top).take(height))
    {
        for (rgba, pixel) in source.chunks_exact(4).zip(row[left * 4..].chunks_exact_mut(4)) {
            let alpha = u32::from(rgba[3]);
            let premultiply = |channel: u8| (u32::from(channel) * alpha + 127) / 255;
            let argb = (alpha << 24)
                | (premultiply(rgba[0]) << 16)
                | (premultiply(rgba[1]) << 8)
                | premultiply(rgba[2]);
            pixel.copy_from_slice(&argb.to_ne_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_dimensions_must_fit_a_nonempty_signed_shm_pool() {
        assert!(icon_layout(0, 0).is_none());
        assert!(icon_layout(0, 1).is_none());
        assert!(icon_layout(1, 0).is_none());
        assert!(icon_layout(u32::MAX, 1).is_none());
        assert!(icon_layout(536_870_912, 1).is_none()); // stride exceeds i32::MAX
        assert!(icon_layout(23_171, 1).is_none()); // padded pool exceeds i32::MAX
        assert!(icon_layout(1, 23_171).is_none());
        let layout = icon_layout(23_170, 23_170).unwrap();
        assert_eq!((layout.edge, layout.stride), (23_170, 92_680));
        let layout = icon_layout(1, 3).unwrap();
        assert_eq!((layout.edge, layout.stride), (3, 12));
    }

    #[test]
    fn icon_pixels_use_native_endian_premultiplied_argb() {
        let mut canvas = [0; 16];
        write_argb8888(
            &[255, 128, 64, 128, 17, 34, 51, 255, 255, 128, 64, 0, 255, 255, 255, 1],
            2,
            2,
            &mut canvas,
        );
        let pixels: Vec<u32> = canvas
            .chunks_exact(4)
            .map(|pixel| u32::from_ne_bytes(pixel.try_into().unwrap()))
            .collect();
        assert_eq!(pixels, [0x80804020, 0xff112233, 0, 0x01010101]);
    }

    #[test]
    fn rectangular_icons_are_padded_to_a_square() {
        let mut canvas = [255; 36];
        write_argb8888(&[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255], 1, 3, &mut canvas);
        let pixels: Vec<u32> = canvas
            .chunks_exact(4)
            .map(|pixel| u32::from_ne_bytes(pixel.try_into().unwrap()))
            .collect();
        assert_eq!(pixels, [0, 0xffff0000, 0, 0, 0xff00ff00, 0, 0, 0xff0000ff, 0]);
    }
}
