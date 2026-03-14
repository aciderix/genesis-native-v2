//! Spatial hash grid for efficient neighbor queries in 2D particle simulations.
//!
//! Uses a packed 20-bit key (10 bits per axis) to hash 2D positions into cells.
//! Supports O(1) insertion and 3×3 neighbor queries for finding nearby particles.

use bevy::prelude::*;
use std::collections::HashMap;

/// Spatial hash grid for O(1) neighbor lookups in 2D.
#[derive(Resource)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<u32, Vec<usize>>,
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    #[inline]
    fn key(x: f32, y: f32, cs: f32) -> u32 {
        let ix = (x / cs).floor() as i32;
        let iy = (y / cs).floor() as i32;
        (((ix as u32) & 0x3FF) << 10) | ((iy as u32) & 0x3FF)
    }

    #[inline]
    fn unpack_key(key: u32) -> (i32, i32) {
        let x = ((key >> 10) & 0x3FF) as i32;
        let y = (key & 0x3FF) as i32;
        (x, y)
    }

    #[inline]
    fn pack_key(cx: i32, cy: i32) -> u32 {
        (((cx as u32) & 0x3FF) << 10) | ((cy as u32) & 0x3FF)
    }

    pub fn clear(&mut self) {
        for bucket in self.cells.values_mut() {
            bucket.clear();
        }
    }

    pub fn insert(&mut self, idx: usize, x: f32, y: f32) {
        let k = Self::key(x, y, self.cell_size);
        self.cells.entry(k).or_insert_with(|| Vec::with_capacity(8)).push(idx);
    }

    pub fn query(&self, x: f32, y: f32) -> Vec<usize> {
        let mut out = Vec::new();
        self.query_into(x, y, &mut out);
        out
    }

    pub fn query_into(&self, x: f32, y: f32, out: &mut Vec<usize>) {
        out.clear();
        let k = Self::key(x, y, self.cell_size);
        let (cx, cy) = Self::unpack_key(k);

        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                let nk = Self::pack_key(cx + dx, cy + dy);
                if let Some(bucket) = self.cells.get(&nk) {
                    out.extend_from_slice(bucket);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query_finds_nearby() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(0, 5.0, 5.0);
        grid.insert(1, 8.0, 8.0);
        grid.insert(2, 500.0, 500.0);

        let neighbors = grid.query(5.0, 5.0);
        assert!(neighbors.contains(&0));
        assert!(neighbors.contains(&1));
        assert!(!neighbors.contains(&2));
    }

    #[test]
    fn clear_removes_all() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(0, 0.0, 0.0);
        grid.clear();
        let neighbors = grid.query(0.0, 0.0);
        assert!(neighbors.is_empty());
    }

    #[test]
    fn query_into_reuses_buffer() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(0, 10.0, 10.0);
        grid.insert(1, 15.0, 15.0);

        let mut buf = Vec::new();
        grid.query_into(12.0, 12.0, &mut buf);
        assert!(buf.contains(&0));
        assert!(buf.contains(&1));
    }
}
