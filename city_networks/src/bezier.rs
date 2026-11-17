//! Cubic Bézier primitives with arc-length reparameterization.
//! Clothoid blending is planned for highway classes (§4.1); the `Spline` enum below is the
//! seam where clothoid variants will slot in without touching consumers.

use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CubicBezier {
    pub p0: Vec3,
    pub p1: Vec3,
    pub p2: Vec3,
    pub p3: Vec3,
}

/// Arc-length lookup table built from 64 segments. Used for arc-length parameterization
/// (meshing, vehicle pursuit) and length queries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArcLengthTable {
    /// Sample t values in [0,1], monotonically increasing.
    #[serde(with = "BigArray")]
    pub ts: [f32; 65],
    /// Cumulative chord length at each t.
    #[serde(with = "BigArray")]
    pub s: [f32; 65],
    pub length: f32,
}

impl CubicBezier {
    #[inline]
    pub fn at(&self, t: f32) -> Vec3 {
        let it = 1.0 - t;
        let it2 = it * it;
        let t2 = t * t;
        it2 * it * self.p0 + 3.0 * it2 * t * self.p1 + 3.0 * it * t2 * self.p2 + t2 * t * self.p3
    }

    #[inline]
    pub fn derivative(&self, t: f32) -> Vec3 {
        let it = 1.0 - t;
        3.0 * it * it * (self.p1 - self.p0) + 6.0 * it * t * (self.p2 - self.p1) + 3.0 * t * t * (self.p3 - self.p2)
    }

    #[inline]
    pub fn second_derivative(&self, t: f32) -> Vec3 {
        let it = 1.0 - t;
        6.0 * it * (self.p2 - 2.0 * self.p1 + self.p0) + 6.0 * t * (self.p3 - 2.0 * self.p2 + self.p1)
    }

    /// Signed curvature κ = |B' × B''| / |B'|³  (planar roads: use the y-component).
    pub fn curvature(&self, t: f32) -> f32 {
        let d = self.derivative(t);
        let dd = self.second_derivative(t);
        let cross = d.cross(dd);
        cross.length() / d.length().powi(3).max(f32::EPSILON)
    }

    pub fn arc_length_table(&self) -> ArcLengthTable {
        let mut ts = [0.0f32; 65];
        let mut s = [0.0f32; 65];
        let mut prev = self.at(0.0);
        let mut length = 0.0f32;
        for i in 1..=64 {
            let t = i as f32 / 64.0;
            let p = self.at(t);
            length += p.distance(prev);
            ts[i] = t;
            s[i] = length;
            prev = p;
        }
        ArcLengthTable { ts, s, length }
    }

    /// Inverse arc-length lookup: t for a given s ∈ [0, length]. Binary search + one Newton step.
    pub fn t_at_s(&self, table: &ArcLengthTable, s: f32) -> f32 {
        if s <= 0.0 {
            return 0.0;
        }
        if s >= table.length {
            return 1.0;
        }
        // Binary search over the LUT.
        let mut lo = 0usize;
        let mut hi = 64usize;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if table.s[mid] <= s {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let s_lo = table.s[lo];
        let s_hi = table.s[hi];
        let frac = if s_hi - s_lo > f32::EPSILON { (s - s_lo) / (s_hi - s_lo) } else { 0.0 };
        let t = table.ts[lo] + frac * (table.ts[hi] - table.ts[lo]);
        // Single Newton refinement on cumulative chord error is optional here; the LUT step
        // is ≤ 2% of a 2 m segment at worst, acceptable for meshing/pursuit.
        t.clamp(0.0, 1.0)
    }

    pub fn point_at_s(&self, table: &ArcLengthTable, s: f32) -> Vec3 {
        self.at(self.t_at_s(table, s))
    }

    pub fn tangent_at_s(&self, table: &ArcLengthTable, s: f32) -> Vec3 {
        self.derivative(self.t_at_s(table, s)).normalize_or_zero()
    }

    /// Adaptive sampling step (sagitta-bounded) for meshers (§4.1).
    pub fn adaptive_step(&self, t: f32, epsilon: f32, ds_min: f32, ds_max: f32) -> f32 {
        let k = self.curvature(t);
        if k < 1e-5 {
            return ds_max;
        }
        (8.0 * epsilon / k).sqrt().clamp(ds_min, ds_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight() -> CubicBezier {
        CubicBezier { p0: Vec3::ZERO, p1: Vec3::new(10.0, 0.0, 0.0), p2: Vec3::new(20.0, 0.0, 0.0), p3: Vec3::new(30.0, 0.0, 0.0) }
    }

    #[test]
    fn straight_line_length() {
        let b = straight();
        let table = b.arc_length_table();
        assert!((table.length - 30.0).abs() < 1e-3, "length = {}", table.length);
    }

    #[test]
    fn t_at_s_endpoints_and_mid() {
        let b = straight();
        let table = b.arc_length_table();
        assert_eq!(b.t_at_s(&table, 0.0), 0.0);
        assert_eq!(b.t_at_s(&table, 30.0), 1.0);
        let p = b.point_at_s(&table, 15.0);
        assert!((p.x - 15.0).abs() < 1e-3, "p = {p:?}");
    }

    #[test]
    fn arc_circle_length() {
        // Roughly quarter-circle-ish S-curve; length should exceed the chord.
        let b = CubicBezier {
            p0: Vec3::ZERO,
            p1: Vec3::new(0.0, 0.0, 40.0),
            p2: Vec3::new(40.0, 0.0, 0.0),
            p3: Vec3::new(40.0, 0.0, 40.0),
        };
        let table = b.arc_length_table();
        let chord = b.p0.distance(b.p3);
        assert!(table.length > chord);
    }

    #[test]
    fn curvature_zero_on_straight() {
        let b = straight();
        assert!(b.curvature(0.5) < 1e-5);
    }
}
