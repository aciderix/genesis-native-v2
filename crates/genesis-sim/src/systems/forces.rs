use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::util::SpatialGrid;
use genesis_core::chemistry::{compute_affinity, motor_force};

/// Forces are chemistry-driven. Uses spatial hashing for O(n) neighbor lookups.
/// Note: SpatialGrid is rebuilt by sensing_system which runs earlier in the chain.
pub fn forces_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    grid: Res<SpatialGrid>,
) {
    let count = store.count;

    // Collect forces in temporary buffer
    let mut fx = vec![0.0f32; count];
    let mut fy = vec![0.0f32; count];

    let mut neighbors = Vec::new();

    // Spatial-hashed pairwise interactions
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }

        grid.query_into(store.x[i], store.y[i], &mut neighbors);

        for &j in &neighbors {
            if j <= i || !store.alive[j] {
                continue;
            }
            let dx = store.x[j] - store.x[i];
            let dy = store.y[j] - store.y[i];
            let dist_sq = dx * dx + dy * dy;
            let r = config.interaction_radius;
            if dist_sq > r * r || dist_sq < 0.001 {
                continue;
            }
            let dist = dist_sq.sqrt();
            let nx = dx / dist;
            let ny = dy / dist;

            // Chemistry-based attraction/repulsion
            let affinity = compute_affinity(&store.chem[i], &store.chem[j]);
            let attraction = affinity * config.force_scale;

            // Short-range repulsion
            let repulsion = if dist < 5.0 {
                config.repulsion_strength * (5.0 - dist) / 5.0
            } else {
                0.0
            };

            let force = attraction / (dist + 1.0) - repulsion;

            fx[i] += force * nx;
            fy[i] += force * ny;
            fx[j] -= force * nx;
            fy[j] -= force * ny;
        }
    }

    // Bond spring forces
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        let bonds_clone = store.bonds[i].clone();
        for &(j, strength) in &bonds_clone {
            if j >= count || !store.alive[j] {
                continue;
            }
            let dx = store.x[j] - store.x[i];
            let dy = store.y[j] - store.y[i];
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 0.001 {
                continue;
            }
            let target_dist = 8.0;
            let force = (dist - target_dist) * config.bond_spring * strength;
            let nx = dx / dist;
            let ny = dy / dist;
            fx[i] += force * nx;
            fy[i] += force * ny;
        }
    }

    // Apply forces
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        let motor = motor_force(&store.chem[i]);
        let angle =
            (store.particle_ids[i] as f32 * 0.1 + store.ages[i] as f32 * 0.01).sin();
        store.vx[i] += fx[i] * config.dt + motor * angle.cos() * config.dt;
        store.vy[i] += fy[i] * config.dt + motor * angle.sin() * config.dt;
    }
}
