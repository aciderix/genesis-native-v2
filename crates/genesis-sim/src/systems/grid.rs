//! Spatial grid rebuild system.
//!
//! Every simulation tick, the spatial hash grid must be cleared and rebuilt
//! from all alive particles. This allows the force, bonding, and neighbor-query
//! systems to efficiently find particles near any given position.

use bevy::prelude::*;
use crate::particle_store::ParticleStore;
use crate::util::SpatialGrid;

/// Rebuild the spatial hash grid from the current particle positions.
///
/// This system runs at the **start** of each simulation tick, before any
/// system that needs neighbor queries (forces, bonds, etc.).
///
/// It clears the grid and re-inserts every alive particle at its current
/// position. Dead particles are skipped — they will be cleaned up separately.
pub fn rebuild_grid_inner(
    grid: &mut SpatialGrid,
    store: &ParticleStore,
) {
    // Clear all grid cells (retains allocated memory for reuse)
    grid.clear();

    // Insert every alive particle into the spatial grid
    let n = store.len();
    for i in 0..n {
        if !store.alive[i] {
            continue;
        }
        grid.insert(i, store.x[i], store.y[i], store.z[i]);
    }
}

pub fn rebuild_grid_system(
    mut grid: ResMut<SpatialGrid>,
    store: Res<ParticleStore>,
) {
    rebuild_grid_inner(&mut *grid, &*store);
}
