use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::SimTick;

/// Integration: apply velocity, then friction.
/// Also increments the global simulation tick counter.
pub fn integrate_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut tick: ResMut<SimTick>,
) {
    // Advance global tick
    tick.0 += 1;

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
