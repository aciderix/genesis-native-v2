use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::SimRng;

/// Predation: larger, more energetic particles can consume smaller ones.
///
/// Conditions for predation:
/// 1. Predator energy > prey energy × min_energy_ratio
/// 2. Distance < predation_radius
/// 3. Predator pays predation_cost
/// 4. Predator gains prey_energy × predation_efficiency
pub fn predation_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut rng: ResMut<SimRng>,
) {
    let count = store.count;
    let r2 = config.predation_radius * config.predation_radius;
    let mut kills: Vec<(usize, usize)> = Vec::new(); // (predator, prey)

    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        // Only attempt predation occasionally (stochastic)
        if rng.next_f32() > 0.01 {
            continue;
        }

        for j in 0..count {
            if i == j || !store.alive[j] {
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
