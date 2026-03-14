use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;

/// World wrapping: keep particles within world bounds.
pub fn wrap_system(mut store: ResMut<ParticleStore>, config: Res<SimConfig>) {
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        if store.x[i] < 0.0 {
            store.x[i] += config.world_width;
        }
        if store.x[i] >= config.world_width {
            store.x[i] -= config.world_width;
        }
        if store.y[i] < 0.0 {
            store.y[i] += config.world_height;
        }
        if store.y[i] >= config.world_height {
            store.y[i] -= config.world_height;
        }
    }
}
