//! A* over the CSR graph with an admissible heuristic.
//!
//! `h(n) = euclidean(n, goal) / max_speed_mps` — admissible because every edge's base cost is at
//! least its length / max_speed and live costs only multiply by factors ≥ 1 (asserted elsewhere).
//! Generation-stamped arrays avoid clearing cost vectors between queries.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use glam::Vec2;

use crate::graph::NetworkGraph;

#[derive(Copy, Clone)]
struct OpenEntry {
    f: f32,
    node: u32,
    g: f32,
}
impl PartialEq for OpenEntry {
    fn eq(&self, other: &Self) -> bool {
        self.f.to_bits() == other.f.to_bits() && self.node == other.node
    }
}
impl Eq for OpenEntry {}
impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OpenEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: BinaryHeap is a max-heap, we want min-f first.
        other.f.total_cmp(&self.f).then_with(|| other.node.cmp(&self.node))
    }
}

/// Returns `(cost, path_including_start_and_goal)` or `None` if unreachable.
pub fn astar(
    graph: &NetworkGraph,
    positions: &[Vec2],
    start: u32,
    goal: u32,
    max_speed_mps: f32,
) -> Option<(f32, Vec<u32>)> {
    let n = graph.node_count();
    if start as usize >= n || goal as usize >= n || positions.len() != n {
        return None;
    }
    if start == goal {
        return Some((0.0, vec![start]));
    }

    let mut g_score = vec![f32::INFINITY; n];
    let mut came_from = vec![u32::MAX; n];
    // Settled-node stamp for this query. h is consistent, so settling is safe.
    // (Per-query allocation is deliberate for now; a batch QueryContext with rolling
    // stamps is the optimization when dispatch moves to worker threads.)
    let mut closed = vec![false; n];

    g_score[start as usize] = 0.0;
    let mut open = BinaryHeap::new();
    let h = |node: u32| positions[node as usize].distance(positions[goal as usize]) / max_speed_mps;
    open.push(OpenEntry { f: h(start), node: start, g: 0.0 });

    while let Some(OpenEntry { node, g, .. }) = open.pop() {
        if closed[node as usize] {
            continue; // stale queue entry
        }
        if g > g_score[node as usize] {
            continue;
        }
        closed[node as usize] = true;

        if node == goal {
            let mut path = vec![goal];
            let mut cur = goal;
            while cur != start {
                cur = came_from[cur as usize];
                if cur == u32::MAX {
                    return None; // corrupt parent chain — should be impossible
                }
                path.push(cur);
            }
            path.reverse();
            return Some((g_score[goal as usize], path));
        }

        for edge in graph.out_edges(node) {
            let to = graph.edge_target(edge);
            if closed[to as usize] {
                continue;
            }
            let tentative = g + graph.edge_cost(edge);
            if tentative < g_score[to as usize] {
                g_score[to as usize] = tentative;
                came_from[to as usize] = node;
                open.push(OpenEntry { f: tentative + h(to), node: to, g: tentative });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphBuilder, LaneNodeKind};

    /// Line graph 0-1-2-3 plus a slow shortcut 0-2.
    fn test_graph() -> (NetworkGraph, Vec<Vec2>) {
        let mut b = GraphBuilder::new(4);
        b.add_edge(0, 1, 10.0, 1800.0);
        b.add_edge(1, 2, 10.0, 1800.0);
        b.add_edge(2, 3, 10.0, 1800.0);
        b.add_edge(1, 0, 10.0, 1800.0);
        b.add_edge(2, 1, 10.0, 1800.0);
        b.add_edge(3, 2, 10.0, 1800.0);
        b.add_edge(0, 2, 50.0, 600.0); // long but high capacity
        let g = b.build();
        let pos = vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0), Vec2::new(200.0, 0.0), Vec2::new(300.0, 0.0)];
        (g, pos)
    }

    #[test]
    fn finds_obvious_path() {
        let (g, pos) = test_graph();
        let (cost, path) = astar(&g, &pos, 0, 3, 30.0).expect("path exists");
        assert_eq!(path, vec![0, 1, 2, 3]);
        assert_eq!(cost, 30.0);
    }

    #[test]
    fn congested_direct_edge_reroutes() {
        let (mut g, pos) = test_graph();
        // Find the 0->2 edge index and congest it heavily.
        let mut e02 = usize::MAX;
        for e in g.out_edges(0) {
            if g.edge_target(e) == 2 {
                e02 = e;
            }
        }
        g.apply_congestion(e02, 5.0); // 50 -> 250, worse than 0-1-2 (20)
        let (_, path) = astar(&g, &pos, 0, 2, 30.0).expect("path exists");
        assert_eq!(path, vec![0, 1, 2]);
    }

    #[test]
    fn unreachable_returns_none() {
        let mut b = GraphBuilder::new(3);
        b.add_edge(0, 1, 10.0, 100.0);
        b.set_node_kind(0, LaneNodeKind::Intersection);
        let g = b.build();
        let pos = vec![Vec2::ZERO, Vec2::X, Vec2::Y];
        assert!(astar(&g, &pos, 0, 2, 30.0).is_none());
    }

    #[test]
    fn zero_cost_same_node() {
        let (g, pos) = test_graph();
        let (cost, path) = astar(&g, &pos, 2, 2, 30.0).expect("identity path");
        assert_eq!(cost, 0.0);
        assert_eq!(path, vec![2]);
    }
}
