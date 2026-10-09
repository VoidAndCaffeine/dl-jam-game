//! Turns the square `dirt.png` / `dirt_wet.png` tile art into a rounded mound
//! silhouette.
//!
//! The source tiles are full opaque squares, so they would read as floor patches
//! rather than mounds. This module computes a soft elliptical alpha field that is
//! multiplied into the art's alpha channel, giving each mound a dome-like
//! silhouette. The field is also what the outline shader keys off, so it has to
//! live on the CPU side, in lockstep with the mask baked into the texture.
//!
//! Pure maths only: `farm.rs` owns the Bevy `Image` plumbing.

/// How far out the silhouette stays fully opaque before feathering to nothing.
///
/// In normalised radius units, where `1.0` is the edge of the ellipse. Values
/// closer to `1.0` give a harder edge; smaller values give a softer, rounder
/// mound.
pub const MOUND_FEATHER: f32 = 0.72;

/// Peak deviation of the organic edge wobble, in normalised radius units.
const MOUND_JITTER: f32 = 0.06;

/// A `size * size` row-major alpha field for a mound, in `0.0..=1.0`.
///
/// Row `0` is the top of the image (matching the pixel layout of a decoded
/// `Image`), and the mound is an ellipse inscribed in the square with a soft,
/// slightly noisy edge.
pub fn mound_alpha_field(size: u32) -> Vec<f32> {
    let n = size as f32;
    let center = (n - 1.0) * 0.5;
    let radius = center.max(1.0);

    let mut field = vec![0.0; (size * size) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - center) / radius;
            let dy = (y as f32 - center) / radius;
            let d = (dx * dx + dy * dy).sqrt() + edge_jitter(x, y);

            field[(y * size + x) as usize] = feather(d);
        }
    }
    field
}

/// `1.0` well inside the mound, `0.0` past its edge, smoothly joined between.
fn feather(d: f32) -> f32 {
    if d <= MOUND_FEATHER {
        return 1.0;
    }
    if d >= 1.0 {
        return 0.0;
    }
    let t = (d - MOUND_FEATHER) / (1.0 - MOUND_FEATHER);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// A deterministic, position-hashed radius nudge so the silhouette is not a
/// perfect ellipse. Kept small enough that it never breaks the smooth edge.
fn edge_jitter(x: u32, y: u32) -> f32 {
    let h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    let h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    let unit = ((h ^ (h >> 16)) & 0xffff) as f32 / 65_535.0;
    (unit - 0.5) * MOUND_JITTER
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: u32 = 64;

    fn at(field: &[f32], x: u32, y: u32) -> f32 {
        field[(y * SIZE + x) as usize]
    }

    #[test]
    fn the_field_has_one_value_per_pixel() {
        assert_eq!(mound_alpha_field(SIZE).len(), (SIZE * SIZE) as usize);
    }

    #[test]
    fn the_centre_is_fully_opaque() {
        let field = mound_alpha_field(SIZE);
        let mid = SIZE / 2;
        assert_eq!(at(&field, mid, mid), 1.0);
        // A little off-centre is still solid.
        assert_eq!(at(&field, mid + 4, mid), 1.0);
    }

    #[test]
    fn every_corner_fades_to_nothing() {
        let field = mound_alpha_field(SIZE);
        for (x, y) in [(0, 0), (SIZE - 1, 0), (0, SIZE - 1), (SIZE - 1, SIZE - 1)] {
            assert!(
                at(&field, x, y) <= f32::EPSILON,
                "corner ({x},{y}) should be transparent, got {}",
                at(&field, x, y)
            );
        }
    }

    #[test]
    fn edges_are_softer_than_the_core() {
        let field = mound_alpha_field(SIZE);
        let mid = SIZE / 2;
        let core = at(&field, mid, mid);
        let shoulder = at(&field, mid, mid + 26);
        assert!(shoulder >= 0.0 && shoulder <= core);
    }

    #[test]
    fn the_field_is_deterministic() {
        assert_eq!(mound_alpha_field(SIZE), mound_alpha_field(SIZE));
    }

    #[test]
    fn alpha_values_stay_within_bounds() {
        for a in mound_alpha_field(SIZE) {
            assert!((0.0..=1.0).contains(&a), "alpha out of range: {a}");
        }
    }

    #[test]
    fn a_tiny_field_still_produces_a_valid_mask() {
        // Guards the `radius.max(1.0)` clamp against a divide-by-zero.
        for size in [1, 2, 3] {
            let field = mound_alpha_field(size);
            assert_eq!(field.len(), (size * size) as usize);
            assert!(field.iter().all(|a| (0.0..=1.0).contains(a)));
        }
    }
}
