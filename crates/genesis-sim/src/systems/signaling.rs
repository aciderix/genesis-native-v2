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

        // Clone the emit queue to avoid borrow conflict
        let emissions: Vec<(usize, f32)> = store.signal_emit[i].clone();
        let mut energy_cost = 0.0f32;

        for &(ch, amount) in &emissions {
            if ch < NUM_CHEMICALS {
                let deposit = amount * config.signal_deposit_rate;
                env.add(ch, gx, gy, deposit);
                energy_cost += deposit * 0.01;
            }
        }

        // Small energy cost for signaling
        store.energy[i] -= energy_cost;
    }
}
