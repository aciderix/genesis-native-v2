use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;

/// Integration: apply velocity, then friction.
pub fn integrate_system(mut store: ResMut<ParticleStore>, config: Res<SimConfig>) {
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        // Apply velocity
        store.x[i] += store.vx[i] * config.dt;
        store.y[i] += store.vy[i] * config.dt;
        // Friction
        store.vx[i] *= config.friction;
        store.vy[i] *= config.friction;
    }
}
