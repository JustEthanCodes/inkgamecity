//! city_terrain — heightfield terrain (§1.3). Pure data; mesh chunking lives in the renderer.

#![forbid(unsafe_code)]

use glam::Vec2;
use noise::{NoiseFn, Perlin};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Regular grid heightfield. `cell_size` in meters; world origin is centered on (0, 0).
#[derive(Clone, Serialize, Deserialize)]
pub struct Heightfield {
    pub cells_x: u32,
    pub cells_y: u32,
    pub cell_size: f32,
    /// Row-major heights, `cells_y * cells_x` entries.
    pub heights: Vec<f32>,
}

impl Heightfield {
    pub fn flat(cells_x: u32, cells_y: u32, cell_size: f32, height: f32) -> Self {
        Self { cells_x, cells_y, cell_size, heights: vec![height; (cells_x * cells_y) as usize] }
    }

    /// Fractal value-noise terrain with a central river valley bias.
    pub fn procedural(cells_x: u32, cells_y: u32, cell_size: f32, seed: u64) -> Self {
        let perlin = Perlin::new(seed as u32);
        let ridged = Perlin::new((seed ^ 0x9E37_79B9) as u32);
        let base_height = 140.0f32;
        let mut hf = Self {
            cells_x,
            cells_y,
            cell_size,
            heights: vec![0.0; (cells_x * cells_y) as usize],
        };
        hf.heights.par_iter_mut().enumerate().for_each(|(i, h)| {
            let x = (i as u32) % cells_x;
            let y = (i as u32) / cells_x;
            let wx = x as f64 * 0.0016;
            let wy = y as f64 * 0.0016;
            let base = perlin.get([wx, wy]) as f32;
            let detail = ridged.get([wx * 3.1, wy * 3.1]) as f32;
            // East-west river valley through the middle (y ≈ cells_y/2).
            let ny = y as f32 / cells_y as f32 - 0.5;
            let valley = 1.0 - (1.0 - (2.6 * ny).abs()).max(0.0);
            let height = base_height * (0.55 + 0.45 * base) + 22.0 * detail.abs()
                - 60.0 * valley.powi(2);
            *h = height.max(2.0);
        });
        hf
    }

    #[inline]
    pub fn world_size(&self) -> Vec2 {
        Vec2::new(self.cells_x as f32 * self.cell_size, self.cells_y as f32 * self.cell_size)
    }

    #[inline]
    fn sample_nearest(&self, x: i32, y: i32) -> f32 {
        let cx = x.clamp(0, self.cells_x as i32 - 1) as usize;
        let cy = y.clamp(0, self.cells_y as i32 - 1) as usize;
        self.heights[cy * self.cells_x as usize + cx]
    }

    /// Bilinear sample at world coordinates (meters, origin at world center).
    pub fn height(&self, world: Vec2) -> f32 {
        let half = self.world_size() * 0.5;
        let gx = ((world.x + half.x) / self.cell_size).clamp(0.0, (self.cells_x - 1) as f32);
        let gy = ((world.y + half.y) / self.cell_size).clamp(0.0, (self.cells_y - 1) as f32);
        let x0 = gx.floor() as i32;
        let y0 = gy.floor() as i32;
        let fx = gx - x0 as f32;
        let fy = gy - y0 as f32;
        let h00 = self.sample_nearest(x0, y0);
        let h10 = self.sample_nearest(x0 + 1, y0);
        let h01 = self.sample_nearest(x0, y0 + 1);
        let h11 = self.sample_nearest(x0 + 1, y0 + 1);
        let top = h00 + (h10 - h00) * fx;
        let bottom = h01 + (h11 - h01) * fx;
        top + (bottom - top) * fy
    }

    /// Approximate world-space normal at a point (central differences, one cell).
    pub fn normal(&self, world: Vec2) -> glam::Vec3 {
        let d = self.cell_size;
        let h_l = self.height(world - Vec2::new(d, 0.0));
        let h_r = self.height(world + Vec2::new(d, 0.0));
        let h_d = self.height(world - Vec2::new(0.0, d));
        let h_u = self.height(world + Vec2::new(0.0, d));
        glam::Vec3::new(h_l - h_r, 2.0 * d, h_d - h_u).normalize()
    }

    pub fn min_max_height(&self) -> (f32, f32) {
        self.heights.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &h| (lo.min(h), hi.max(h)))
    }

    /// Cut-and-fill blend (§4.2 step 3). Brings terrain toward `road_height` near the
    /// centerline of the road corridor with a smoothstep over `influence` meters.
    /// `distance` is the signed 2D distance to the centerline (computed by the road system).
    pub fn apply_road_corridor(
        &mut self,
        corridor: &[(Vec2, f32, f32)], // (point, road_height, influence)
    ) {
        for &(center, road_h, influence) in corridor {
            if influence <= 0.0 {
                continue;
            }
            let half = self.world_size() * 0.5;
            // World → grid: cell i covers world [i*cell - half, (i+1)*cell - half)
            let x0 = (((center.x - influence) + half.x) / self.cell_size).floor().max(0.0) as i32;
            let y0 = (((center.y - influence) + half.y) / self.cell_size).floor().max(0.0) as i32;
            let x1 = (((center.x + influence) + half.x) / self.cell_size).ceil()
                .min((self.cells_x - 1) as f32) as i32;
            let y1 = (((center.y + influence) + half.y) / self.cell_size).ceil()
                .min((self.cells_y - 1) as f32) as i32;
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    let world = Vec2::new(
                        cx as f32 * self.cell_size - half.x,
                        cy as f32 * self.cell_size - half.y,
                    );
                    let d = world.distance(center);
                    if d >= influence {
                        continue;
                    }
                    let t = city_core::smoothstep(0.0, influence, influence - d);
                    let i = cy as usize * self.cells_x as usize + cx as usize;
                    let original = self.heights[i];
                    // Blend toward road elevation including a 1:1.5 side slope offset.
                    let target = road_h + (d * (1.0 / 1.5)).min(6.0);
                    self.heights[i] = original + (target - original) * t;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_heightfield_samples_constant() {
        let hf = Heightfield::flat(16, 16, 4.0, 42.0);
        assert_eq!(hf.height(Vec2::new(0.0, 0.0)), 42.0);
        assert_eq!(hf.height(Vec2::new(31.9, -31.9)), 42.0);
    }

    #[test]
    fn procedural_is_deterministic() {
        let a = Heightfield::procedural(64, 64, 4.0, 1337);
        let b = Heightfield::procedural(64, 64, 4.0, 1337);
        assert_eq!(a.heights, b.heights);
        let (lo, hi) = a.min_max_height();
        assert!(lo >= 2.0 && hi > 20.0);
    }

    #[test]
    fn bilinear_interpolates_corners() {
        // Two-cell-wide ramp: heights 0 and 10 along x.
        let hf = Heightfield { cells_x: 2, cells_y: 1, cell_size: 10.0, heights: vec![0.0, 10.0] };
        let h = hf.height(Vec2::new(-5.0, 0.0)); // halfway between the two grid points
        assert!((h - 5.0).abs() < 1e-4, "h = {h}");
    }

    #[test]
    fn corridor_raises_fill_and_keeps_far_cells() {
        let mut hf = Heightfield::flat(32, 32, 2.0, 10.0);
        // Road crossing through (0,0) at elevation 30 with 8 m influence.
        hf.apply_road_corridor(&[(Vec2::new(0.0, 0.0), 30.0, 8.0)]);
        let center = hf.height(Vec2::new(0.0, 0.0));
        assert!(center > 25.0, "center filled to {center}");
        let far = hf.height(Vec2::new(31.0, 31.0));
        assert_eq!(far, 10.0, "out-of-influence cells untouched");
    }
}
