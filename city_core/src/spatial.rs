//! Uniform grid hash with intrusive slab lists — O(1) insert/remove/update for dynamic agents.
//!
//! Used for vehicles and pedestrians (§2.6.1 of the design doc). Static lots use a loose
//! quadtree (not yet implemented); visibility culling is GPU-driven.
//!
//! Entities are stored as raw `u64` so this crate stays engine-free. `GridHandle` is a
//! generational index into the slab; stale handles are rejected (panic in debug, no-op in release).

use rustc_hash::FxHashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AgentLayer {
    Vehicle,
    Pedestrian,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GridHandle {
    index: u32,
    generation: u32,
}
impl GridHandle {
    #[inline]
    pub const fn index(self) -> usize {
        self.index as usize
    }
}

const NONE: u32 = u32::MAX;

#[derive(Copy, Clone)]
struct Entry {
    entity: u64,
    x: f32,
    y: f32,
    layer: AgentLayer,
    generation: u32,
    alive: bool,
    next: u32,
    prev: u32,
    cell: u64,
}

pub struct SpatialGrid {
    cell_size: f32,
    head: FxHashMap<u64, u32>,
    slab: Vec<Entry>,
    free: Vec<u32>,
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        assert!(cell_size > 0.0 && cell_size.is_finite());
        Self { cell_size, head: FxHashMap::default(), slab: Vec::new(), free: Vec::new() }
    }

    #[inline]
    fn cell_of(&self, p: (f32, f32)) -> (i32, i32) {
        ((p.0 / self.cell_size).floor() as i32, (p.1 / self.cell_size).floor() as i32)
    }

    #[inline]
    fn key(c: (i32, i32)) -> u64 {
        // Offset into unsigned space so negatives pack cleanly.
        let x = (c.0 as i64 + (1i64 << 31)) as u64;
        let y = (c.1 as i64 + (1i64 << 31)) as u64;
        (y << 32) | (x & 0xFFFF_FFFF)
    }

    pub fn len(&self) -> usize {
        self.slab.len() - self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn entry(&self, h: GridHandle) -> Option<&Entry> {
        let e = self.slab.get(h.index as usize)?;
        (e.alive && e.generation == h.generation).then_some(e)
    }

    pub fn insert(&mut self, entity: u64, pos: (f32, f32), layer: AgentLayer) -> GridHandle {
        let (cx, cy) = self.cell_of(pos);
        let key = Self::key((cx, cy));
        let old_head = self.head.get(&key).copied().unwrap_or(NONE);

        let index = match self.free.pop() {
            Some(i) => i as usize,
            None => {
                self.slab.push(Entry {
                    entity: 0, x: 0.0, y: 0.0,
                    layer, generation: 0, alive: false,
                    next: NONE, prev: NONE, cell: key,
                });
                self.slab.len() - 1
            }
        };

        let generation = self.slab[index].generation;
        self.slab[index] = Entry {
            entity, x: pos.0, y: pos.1, layer, generation,
            alive: true, next: old_head, prev: NONE, cell: key,
        };
        if old_head != NONE {
            self.slab[old_head as usize].prev = index as u32;
        }
        self.head.insert(key, index as u32);
        GridHandle { index: index as u32, generation }
    }

    fn unlink(&mut self, index: usize) {
        let e = self.slab[index];
        if e.prev != NONE {
            self.slab[e.prev as usize].next = e.next;
        } else if self.head.get(&e.cell).copied() == Some(index as u32) {
            self.head.remove(&e.cell);
        } else {
            // Head moved after unlink ordering — fall back to removing stale key if present.
            self.head.retain(|_, v| *v != index as u32);
        }
        if e.next != NONE {
            self.slab[e.next as usize].prev = e.prev;
        }
    }

    /// Remove an entry. Returns `false` without side effects for stale handles
    /// (gameplay code may see replication races); never panics.
    pub fn remove(&mut self, h: GridHandle) -> bool {
        if self.entry(h).is_none() {
            return false;
        }
        let index = h.index as usize;
        self.unlink(index);
        let e = &mut self.slab[index];
        e.alive = false;
        e.entity = 0;
        e.generation = e.generation.wrapping_add(1);
        self.free.push(index as u32);
        true
    }

    /// Move an entry. Stale handles are a programmer error: debug_assert in dev builds,
    /// silently ignored in release (same robustness policy as `remove`).
    pub fn update(&mut self, h: GridHandle, pos: (f32, f32)) {
        if self.slab.get(h.index as usize).is_none_or(|e| !e.alive || e.generation != h.generation) {
            debug_assert!(false, "stale SpatialGrid handle");
            return;
        }
        let index = h.index as usize;
        self.slab[index].x = pos.0;
        self.slab[index].y = pos.1;
        let (cx, cy) = self.cell_of(pos);
        let key = Self::key((cx, cy));
        if key != self.slab[index].cell {
            self.unlink(index);
            let old_head = self.head.get(&key).copied().unwrap_or(NONE);
            // entry(h) validated above; amend in place.
            let e = &mut self.slab[index];
            e.next = old_head;
            e.prev = NONE;
            e.cell = key;
            if old_head != NONE {
                self.slab[old_head as usize].prev = index as u32;
            }
            self.head.insert(key, index as u32);
        }
    }

    fn scan_cells(&self, min: (f32, f32), max: (f32, f32), layer: Option<AgentLayer>, out: &mut Vec<(u64, f32, f32)>) {
        let (x0, y0) = self.cell_of(min);
        let (x1, y1) = self.cell_of(max);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let key = Self::key((cx, cy));
                let mut cur = self.head.get(&key).copied().unwrap_or(NONE);
                while cur != NONE {
                    let e = &self.slab[cur as usize];
                    if layer.is_none_or(|l| l == e.layer) {
                        out.push((e.entity, e.x, e.y));
                    }
                    cur = e.next;
                }
            }
        }
    }

    /// Axis-aligned rect query (bounds inclusive).
    pub fn query_rect(&self, min: (f32, f32), max: (f32, f32), layer: Option<AgentLayer>) -> Vec<u64> {
        let mut raw = Vec::new();
        self.scan_cells(min, max, layer, &mut raw);
        raw.into_iter()
            .filter(|&(_, x, y)| x >= min.0 && x <= max.0 && y >= min.1 && y <= max.1)
            .map(|(e, _, _)| e)
            .collect()
    }

    /// Circle query with true radial test.
    pub fn query_circle(&self, center: (f32, f32), radius: f32, layer: Option<AgentLayer>) -> Vec<u64> {
        let mut raw = Vec::new();
        self.scan_cells(
            (center.0 - radius, center.1 - radius),
            (center.0 + radius, center.1 + radius),
            layer,
            &mut raw,
        );
        let r2 = radius * radius;
        raw.into_iter()
            .filter(|&(_, x, y)| {
                let dx = x - center.0;
                let dy = y - center.1;
                dx * dx + dy * dy <= r2
            })
            .map(|(e, _, _)| e)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_query_remove_cycle() {
        let mut g = SpatialGrid::new(16.0);
        let h1 = g.insert(1, (1.0, 1.0), AgentLayer::Vehicle);
        let h2 = g.insert(2, (5.0, 5.0), AgentLayer::Pedestrian);
        let h3 = g.insert(3, (500.0, 500.0), AgentLayer::Vehicle);
        assert_eq!(g.len(), 3);

        let near = g.query_rect((0.0, 0.0), (10.0, 10.0), Some(AgentLayer::Vehicle));
        assert_eq!(near, vec![1]);
        let all = g.query_rect((0.0, 0.0), (10.0, 10.0), None);
        assert_eq!(all.len(), 2);

        g.update(h2, (1.5, 1.5)); // move pedestrian into the vehicle cell
        assert_eq!(g.query_circle((1.0, 1.0), 2.0, Some(AgentLayer::Vehicle)), vec![1]);
        assert_eq!(g.query_circle((1.0, 1.0), 2.0, None).len(), 2);

        assert!(g.remove(h1));
        assert!(g.remove(h3));
        assert!(g.remove(h2));
        assert_eq!(g.len(), 0);
    }

    #[test]
    fn stale_handle_rejected() {
        let mut g = SpatialGrid::new(16.0);
        let h = g.insert(7, (0.0, 0.0), AgentLayer::Vehicle);
        assert!(g.remove(h));
        assert!(!g.remove(h)); // double remove: no panic in release, `false`
    }

    #[test]
    fn negative_world_coordinates() {
        let mut g = SpatialGrid::new(16.0);
        g.insert(42, (-500.5, -1200.25), AgentLayer::Vehicle);
        assert_eq!(g.query_rect((-510.0, -1210.0), (-490.0, -1190.0), None), vec![42]);
        assert_eq!(g.query_circle((-500.0, -1200.0), 2.0, None), vec![42]);
    }

    #[test]
    fn slab_reuse_keeps_len_bounded() {
        let mut g = SpatialGrid::new(16.0);
        let mut handles = Vec::new();
        for i in 0..10_000u64 {
            handles.push(g.insert(i, ((i % 100) as f32 * 3.0, (i / 100) as f32 * 3.0), AgentLayer::Vehicle));
        }
        assert_eq!(g.len(), 10_000);
        assert_eq!(g.slab.len(), 10_000);
        for h in handles.drain(..) {
            g.remove(h);
        }
        for i in 0..10_000u64 {
            handles.push(g.insert(100_000 + i, (1.0, 1.0), AgentLayer::Vehicle));
        }
        assert_eq!(g.slab.len(), 10_000, "recycled slab slots were reused");
    }

    #[test]
    fn circle_excludes_far_corners() {
        let mut g = SpatialGrid::new(4.0);
        g.insert(1, (7.9, 7.9), AgentLayer::Vehicle); // inside bbox, outside circle
        g.insert(2, (2.0, 2.0), AgentLayer::Vehicle);
        let hits = g.query_circle((0.0, 0.0), 4.0, None);
        assert!(hits.contains(&2) && !hits.contains(&1));
    }
}
