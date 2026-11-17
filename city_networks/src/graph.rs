//! CSR (compressed sparse row) directed lane graph with live congestion weights (§2.6.4).
//!
//! Nodes are lane-transition points; edges carry base (free-flow) cost, an absolute
//! congestion refresh (overwritten by the 4 Hz sampler) and a persistent condition factor
//! (weather/surface, e.g. wet road 1.0–2.0). Live cost = base × congestion × condition.
//!
//! CCH (customizable contraction hierarchies) preprocessing is intentionally not in this
//! vertical slice; A* with an admissible heuristic (base-cost lower bound) is used now.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LaneNodeKind {
    Intersection = 0,
    Signalized = 1,
    Merge = 2,
    Crosswalk = 3,
    Ramp = 4,
}

#[derive(Default)]
pub struct GraphBuilder {
    node_count: u32,
    node_kind: Vec<LaneNodeKind>,
    edges: Vec<(u32, u32, f32, f32)>, // (from, to, base_cost, capacity_veh_per_hour)
}

impl GraphBuilder {
    pub fn new(node_count: u32) -> Self {
        Self { node_count, node_kind: vec![LaneNodeKind::Intersection; node_count as usize], edges: Vec::new() }
    }

    pub fn set_node_kind(&mut self, node: u32, kind: LaneNodeKind) {
        self.node_kind[node as usize] = kind;
    }

    /// Adds a directed edge. Both directions of a two-way road need two calls.
    pub fn add_edge(&mut self, from: u32, to: u32, base_cost: f32, capacity_veh_per_hour: f32) {
        assert!(from < self.node_count && to < self.node_count);
        assert!(base_cost.is_finite() && base_cost >= 0.0);
        self.edges.push((from, to, base_cost, capacity_veh_per_hour));
    }

    pub fn build(self) -> NetworkGraph {
        let n = self.node_count as usize;
        let mut out_deg = vec![0u32; n + 1];
        for &(from, _, _, _) in &self.edges {
            out_deg[from as usize + 1] += 1;
        }
        for i in 1..=n {
            out_deg[i] += out_deg[i - 1];
        }
        let edge_count = self.edges.len();
        let mut cursor = out_deg.clone();
        let mut edge_target = Vec::with_capacity(edge_count);
        let mut edge_cost_base = Vec::with_capacity(edge_count);
        let mut edge_cost_live = Vec::with_capacity(edge_count);
        let mut edge_capacity = Vec::with_capacity(edge_count);
        let mut slot_of_edge = vec![0usize; edge_count];
        for (i, &(from, _, _, _)) in self.edges.iter().enumerate() {
            slot_of_edge[i] = cursor[from as usize] as usize;
            cursor[from as usize] += 1;
        }
        let mut targets = vec![0u32; edge_count];
        let mut bases = vec![0f32; edge_count];
        let mut caps = vec![0f32; edge_count];
        for (i, &(_, to, base, cap)) in self.edges.iter().enumerate() {
            let slot = slot_of_edge[i];
            targets[slot] = to;
            bases[slot] = base;
            caps[slot] = cap;
        }
        edge_target.extend_from_slice(&targets);
        edge_cost_base.extend_from_slice(&bases);
        edge_cost_live.extend_from_slice(&bases);
        edge_capacity.extend_from_slice(&caps);

        NetworkGraph {
            node_kind: self.node_kind,
            node_offsets: out_deg,
            edge_target,
            edge_cost_base,
            edge_cost_live,
            edge_congestion: vec![1.0; edge_count],
            edge_condition: vec![1.0; edge_count],
            edge_capacity,
            live_version: 0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkGraph {
    pub node_kind: Vec<LaneNodeKind>,
    /// CSR offsets; edges of node i are `edge_target[offsets[i]..offsets[i+1]]`.
    pub node_offsets: Vec<u32>,
    pub edge_target: Vec<u32>,
    pub edge_cost_base: Vec<f32>,
    /// Live (congestion/condition/toll-scaled) costs; always >= base costs.
    pub edge_cost_live: Vec<f32>,
    pub edge_capacity: Vec<f32>,
    /// Bumped on any live-cost write; pathfinding jobs compare against snapshot version.
    pub live_version: u64,
    edge_congestion: Vec<f32>,
    edge_condition: Vec<f32>,
}

impl NetworkGraph {
    pub fn node_count(&self) -> usize {
        self.node_kind.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edge_target.len()
    }

    #[inline]
    pub fn out_edges(&self, node: u32) -> std::ops::Range<usize> {
        let i = node as usize;
        self.node_offsets[i] as usize..self.node_offsets[i + 1] as usize
    }

    #[inline]
    pub fn edge_target(&self, edge: usize) -> u32 {
        self.edge_target[edge]
    }

    #[inline]
    pub fn edge_cost(&self, edge: usize) -> f32 {
        self.edge_cost_live[edge]
    }

    /// Set congestion factor ≥ 1.0 (absolute refresh from the 4 Hz density sampler).
    pub fn apply_congestion(&mut self, edge: usize, factor: f32) {
        assert!(factor.is_finite() && factor >= 1.0);
        self.edge_congestion[edge] = factor;
        self.edge_cost_live[edge] = self.edge_cost_base[edge] * factor * self.edge_condition[edge];
        self.live_version += 1;
    }

    /// Set persistent condition factor ≥ 1.0 (weather/surface). Does not compound.
    pub fn apply_condition(&mut self, edge: usize, factor: f32) {
        assert!(factor.is_finite() && factor >= 1.0);
        self.edge_condition[edge] = factor;
        self.edge_cost_live[edge] = self.edge_cost_base[edge] * self.edge_congestion[edge] * factor;
        self.live_version += 1;
    }

    /// Reset every live cost to base. O(E); called after full rebuilds, not per frame.
    pub fn reset_live_costs(&mut self) {
        self.edge_congestion.fill(1.0);
        self.edge_condition.fill(1.0);
        self.edge_cost_live.copy_from_slice(&self.edge_cost_base);
        self.live_version += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csr_layout_groups_by_source() {
        let mut b = GraphBuilder::new(4);
        b.add_edge(0, 1, 10.0, 1800.0);
        b.add_edge(0, 2, 20.0, 1800.0);
        b.add_edge(1, 3, 5.0, 1800.0);
        b.add_edge(2, 3, 5.0, 1800.0);
        let g = b.build();
        assert_eq!(g.node_offsets, vec![0, 2, 3, 4, 4]);
        assert_eq!(&g.edge_target[0..2], &[1, 2]);
    }

    #[test]
    fn congestion_bumps_version() {
        let mut b = GraphBuilder::new(2);
        b.add_edge(0, 1, 10.0, 1800.0);
        let mut g = b.build();
        let v = g.live_version;
        g.apply_congestion(0, 2.5);
        assert_eq!(g.edge_cost(0), 25.0);
        assert_eq!(g.live_version, v + 1);
        g.reset_live_costs();
        assert_eq!(g.edge_cost(0), 10.0);
    }
}
