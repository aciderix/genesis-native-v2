use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::SimRng;
use crate::util::spatial_grid::SpatialGrid;

/// Predation: larger, more energetic particles can consume smaller ones.
///
/// Uses the SpatialGrid from sensing_system for O(n) neighbor queries
/// instead of O(n²) brute force.
pub fn predation_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut rng: ResMut<SimRng>,
    grid: Res<SpatialGrid>,
) {
    let count = store.count;
    let r2 = config.predation_radius * config.predation_radius;
    let mut kills: Vec<(usize, usize)> = Vec::new(); // (predator, prey)
    let mut neighbors_buf = Vec::new();

    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        // Only attempt predation occasionally (stochastic)
        if rng.next_f32() > 0.01 {
            continue;
        }

        // Use spatial grid for neighbor query instead of scanning all particles
        neighbors_buf.clear();
        grid.query(store.x[i], store.y[i], config.predation_radius, &mut neighbors_buf);

        for &j in &neighbors_buf {
            if j >= count || i == j || !store.alive[j] {
                continue;
            }
            // Already scheduled to die?
            if kills.iter().any(|&(_, prey)| prey == j) {
                continue;
            }

            let dx = store.x[j] - store.x[i];
            let dy = store.y[j] - store.y[i];
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > r2 {
                continue;
            }

            // Energy ratio check
            if store.energy[i] > store.energy[j] * config.predation_min_energy_ratio {
                kills.push((i, j));
                break; // One kill per tick per predator
            }
        }
    }

    // Apply kills
    for (predator, prey) in &kills {
        let prey_energy = store.energy[*prey];
        store.energy[*predator] += prey_energy * config.predation_efficiency;
        store.energy[*predator] -= config.predation_cost;
        store.energy[*predator] = store.energy[*predator].clamp(0.0, config.max_energy);
        store.alive[*prey] = false;
    }
}
