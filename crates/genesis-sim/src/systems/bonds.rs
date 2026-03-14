use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::SimRng;
use genesis_core::chemistry::compute_bond_strength;

/// Bond formation/breaking based on chemistry affinity.
pub fn bonds_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut rng: ResMut<SimRng>,
) {
    let count = store.count;

    // Bond formation: check nearby unbonded pairs
    let mut new_bonds: Vec<(usize, usize, f32)> = Vec::new();
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        if store.bonds[i].len() >= 4 {
            continue;
        }
        for j in (i + 1)..count {
            if !store.alive[j] {
                continue;
            }
            if store.bonds[j].len() >= 4 {
                continue;
            }
            // Check if already bonded
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

    // Bond breaking: bonds weaken and break
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
            // Break if too far or too weak
            let current_strength = compute_bond_strength(&store.chem[i], &store.chem[j]);
            if dist > config.bond_max_distance * 2.0 || current_strength < 0.15 {
                to_remove.push(b);
            } else {
                // Update strength
                store.bonds[i][b].1 = current_strength;
            }
        }
        // Remove broken bonds (reverse order to preserve indices)
        for &b in to_remove.iter().rev() {
            let (j, _) = store.bonds[i][b];
            store.bonds[i].swap_remove(b);
            // Also remove from partner
            if j < count {
                if let Some(pos) = store.bonds[j].iter().position(|&(p, _)| p == i) {
                    store.bonds[j].swap_remove(pos);
                }
            }
        }
    }
}
