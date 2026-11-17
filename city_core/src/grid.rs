//! Dense chunked f32 field (land value, pollution, water depth, ...).
//! Chunks carry a dirty bitmap so incremental systems (SWE, diffusion, save journal)
//! only touch changed cells. 128×128 cells per chunk, one bit per cell.

use serde::{Deserialize, Serialize};

pub const CHUNK_CELLS: usize = 128;
pub const CHUNK_BITS: usize = CHUNK_CELLS * CHUNK_CELLS / 64;

#[derive(Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub cells: Vec<f32>,      // always CHUNK_CELLS*CHUNK_CELLS
    pub dirty: Vec<u64>,      // CHUNK_BITS bits
}

impl Chunk {
    fn fresh() -> Self {
        Self { cells: vec![0.0; CHUNK_CELLS * CHUNK_CELLS], dirty: vec![0; CHUNK_BITS] }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChunkedGrid {
    pub cell_size: f32,
    /// Width/height in *cells* (not chunks).
    pub width: usize,
    pub height: usize,
    chunks: Vec<Chunk>, // row-major over chunk grid
    pub chunks_x: usize,
    pub chunks_y: usize,
}

impl ChunkedGrid {
    pub fn new(width: usize, height: usize, cell_size: f32) -> Self {
        let chunks_x = width.div_ceil(CHUNK_CELLS);
        let chunks_y = height.div_ceil(CHUNK_CELLS);
        Self {
            cell_size,
            width,
            height,
            chunks: (0..chunks_x * chunks_y).map(|_| Chunk::fresh()).collect(),
            chunks_x,
            chunks_y,
        }
    }

    #[inline]
    fn locate(&self, x: usize, y: usize) -> (usize, usize, usize) {
        let cxi = x / CHUNK_CELLS;
        let cyi = y / CHUNK_CELLS;
        let lx = x % CHUNK_CELLS;
        let ly = y % CHUNK_CELLS;
        (cyi * self.chunks_x + cxi, ly * CHUNK_CELLS + lx, ly * CHUNK_CELLS + lx)
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x >= self.width || y >= self.height {
            return 0.0;
        }
        let (chunk, i, _) = self.locate(x, y);
        self.chunks[chunk].cells[i]
    }

    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let (chunk, i, _) = self.locate(x, y);
        let c = &mut self.chunks[chunk];
        c.cells[i] = v;
        let word = i / 64;
        c.dirty[word] |= 1u64 << (i % 64);
    }

    /// Clear all dirty bitmaps (call after journal flush or save).
    pub fn clear_dirty(&mut self) {
        for c in &mut self.chunks {
            c.dirty.iter_mut().for_each(|w| *w = 0);
        }
    }

    /// Iterate set-dirty cell coordinates. Call `clear_dirty` when done.
    pub fn for_each_dirty(&self, mut f: impl FnMut(usize, usize)) {
        for cyi in 0..self.chunks_y {
            for cxi in 0..self.chunks_x {
                let chunk = &self.chunks[cyi * self.chunks_x + cxi];
                for (wi, &word) in chunk.dirty.iter().enumerate() {
                    if word == 0 {
                        continue;
                    }
                    let mut w = word;
                    while w != 0 {
                        let bit = w.trailing_zeros() as usize;
                        w &= w - 1;
                        let i = wi * 64 + bit;
                        let lx = i % CHUNK_CELLS;
                        let ly = i / CHUNK_CELLS;
                        f(cxi * CHUNK_CELLS + lx, cyi * CHUNK_CELLS + ly);
                    }
                }
            }
        }
    }

    /// Max value iterator helper for diagnostics/tests.
    pub fn max_value(&self) -> f32 {
        self.chunks
            .iter()
            .flat_map(|c| c.cells.iter())
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_roundtrip_and_dirty() {
        let mut g = ChunkedGrid::new(300, 200, 4.0);
        g.set(0, 0, 1.5);
        g.set(299, 199, 2.5);
        g.set(128, 129, 3.5); // second chunk over
        assert_eq!(g.get(0, 0), 1.5);
        assert_eq!(g.get(299, 199), 2.5);
        assert_eq!(g.get(128, 129), 3.5);

        let mut dirty = Vec::new();
        g.for_each_dirty(|x, y| dirty.push((x, y)));
        assert_eq!(dirty.len(), 3);
        g.clear_dirty();
        let mut again = Vec::new();
        g.for_each_dirty(|x, y| again.push((x, y)));
        assert!(again.is_empty());
    }

    #[test]
    fn out_of_bounds_is_safe() {
        let mut g = ChunkedGrid::new(10, 10, 1.0);
        g.set(10, 0, 9.9); // ignored
        assert_eq!(g.get(10, 0), 0.0);
    }
}
