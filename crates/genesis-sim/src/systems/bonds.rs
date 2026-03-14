use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::SimRng;
use crate::util::SpatialGrid;
use genesis_core::chemistry::compute_bond_strength;

/// Bond formation/breaking based on chemistry affinity. Uses spatial hashing.
pub fn bonds_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut rng: ResMut<SimRng>,
    grid: Res<SpatialGrid>,
) {
    let count = store.count;
    let mut neighbors = Vec::new();

    // Bond formation: check nearby unbonded pairs using spatial grid
    let mut new_bonds: Vec<(usize, usize, f32)> = Vec::new();
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        if store.bonds[i].len() >= 4 {
            continue;
        }

        grid.query_into(store.x[i], store.y[i], &mut neighbors);

        for &j in &neighbors {
            if j <= i || !store.alive[j] {
                continue;
            }
            if store.bonds[j].len() >= 4 {
                continue;
            }
            if store.bonds[i].iter().any(|&(p, _)| p == j) {
                continue;
            }

            let dx = store.x[j] - store.x[i];
            let dy = store.y[j] - store.y[i];
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > config.bond_max_distance {
                continue;
            }

            let strength = compute_bond_strength(&store.chem[i], &store.chem[j]);
            if strength > 0.3 && rng.next_f32() < strength * 0.1 {
                new_bonds.push((i, j, strength));
            }
        }
    }

    // Apply new bonds
    for (i, j, strength) in new_bonds {
        store.bonds[i].push((j, strength));
        store.bonds[j].push((i, strength));
    }

    // Bond breaking
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        let mut to_remove: Vec<usize> = Vec::new();
        for b in 0..store.bonds[i].len() {
            let (j, _strength) = store.bonds[i][b];
            if j >= count || !store.alive[j] {
                to_remove.push(b);
                continue;
            }
            let dx = store.x[j] - store.x[i];
            let dy = store.y[j] - store.y[i];
            let dist = (dx * dx + dy * dy).sqrt();
            let current_strength = compute_bond_strength(&store.chem[i], &store.chem[j]);
            if dist > config.bond_max_distance * 2.0 || current_strength < 0.15 {
                to_remove.push(b);
            } else {
                store.bonds[i][b].1 = current_strength;
            }
        }
        for &b in to_remove.iter().rev() {
            let (j, _) = store.bonds[i][b];
            store.bonds[i].swap_remove(b);
            if j < count {
                if let Some(pos) = store.bonds[j].iter().position(|&(p, _)| p == i) {
                    store.bonds[j].swap_remove(pos);
                }
            }
        }
    }
}
