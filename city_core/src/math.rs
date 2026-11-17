//! Small math accelerators: oriented bounding boxes, AABBs, easing helpers.

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 2D oriented bounding box (lot interaction zone, lot fit via rotating calipers upstream).
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Obb2 {
    pub center: Vec2,
    pub half_extents: Vec2,
    /// Rotation in radians, CCW around +Z.
    pub rotation: f32,
}

impl Obb2 {
    #[inline]
    pub fn contains(&self, p: Vec2) -> bool {
        let d = p - self.center;
        let (s, c) = self.rotation.sin_cos();
        // Rotate into the box frame.
        let local = Vec2::new(c * d.x + s * d.y, -s * d.x + c * d.y);
        local.x.abs() <= self.half_extents.x && local.y.abs() <= self.half_extents.y
    }

    #[inline]
    pub fn area(&self) -> f32 {
        4.0 * self.half_extents.x * self.half_extents.y
    }
}

/// 3D AABB for picking, broadphase and BVH nodes.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Aabb3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb3 {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        debug_assert!(min.x <= max.x && min.y <= max.y && min.z <= max.z);
        Self { min, max }
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    #[inline]
    pub fn extents(&self) -> Vec3 {
        self.max - self.min
    }

    #[inline]
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    #[inline]
    pub fn intersects(&self, other: &Aabb3) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    #[inline]
    pub fn union(&self, other: &Aabb3) -> Aabb3 {
        Aabb3 { min: self.min.min(other.min), max: self.max.max(other.max) }
    }

    #[inline]
    pub fn expanded(&self, margin: f32) -> Aabb3 {
        Aabb3 { min: self.min - Vec3::splat(margin), max: self.max + Vec3::splat(margin) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obb_rotated_contains() {
        // 4x2 box rotated 90°: world axis swapped.
        let obb = Obb2 { center: Vec2::ZERO, half_extents: Vec2::new(2.0, 1.0), rotation: std::f32::consts::FRAC_PI_2 };
        assert!(obb.contains(Vec2::new(0.5, 1.9)));
        assert!(!obb.contains(Vec2::new(1.9, 0.5)));
    }

    #[test]
    fn obb_axis_aligned() {
        let obb = Obb2 { center: Vec2::new(10.0, 10.0), half_extents: Vec2::new(5.0, 5.0), rotation: 0.0 };
        assert!(obb.contains(Vec2::new(14.9, 5.1)));
        assert!(!obb.contains(Vec2::new(15.1, 10.0)));
    }

    #[test]
    fn aabb_union() {
        let a = Aabb3::new(Vec3::ZERO, Vec3::ONE);
        let b = Aabb3::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(3.0, 2.0, 1.0));
        let u = a.union(&b);
        assert_eq!(u.max, Vec3::new(3.0, 2.0, 1.0));
    }

    #[test]
    fn smoothstep_endpoints() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }
}
