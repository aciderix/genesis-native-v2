use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::EnvironmentFields;
use genesis_core::chemistry::NUM_CHEMICALS;

/// Process signal emissions from the behavior system.
///
/// Particles deposit chemicals into the environment grid based on
/// their behavior-driven signal_emit queues.
pub fn signaling_system(
    mut store: ResMut<ParticleStore>,
    mut env: ResMut<EnvironmentFields>,
    config: Res<SimConfig>,
) {
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        if store.signal_emit[i].is_empty() {
            continue;
        }

        let (gx, gy) = env.world_to_grid(store.x[i], store.y[i]);

        for &(ch, amount) in &store.signal_emit[i] {
            if ch < NUM_CHEMICALS {
                let deposit = amount * config.signal_deposit_rate;
                env.add(ch, gx, gy, deposit);

                // Small energy cost for signaling
                store.energy[i] -= deposit * 0.01;
            }
        }
    }
}
