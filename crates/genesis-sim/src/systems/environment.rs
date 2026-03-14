use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::EnvironmentFields;
use genesis_core::chemistry::NUM_CHEMICALS;

/// Particle-environment chemical exchange + field diffusion/decay.
pub fn environment_system(
    mut store: ResMut<ParticleStore>,
    mut env: ResMut<EnvironmentFields>,
    config: Res<SimConfig>,
) {
    // Particle absorption and excretion
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        let (gx, gy) = env.world_to_grid(store.x[i], store.y[i]);
        for k in 0..NUM_CHEMICALS {
            let env_val = env.get(k, gx, gy);
            // Absorption: take from environment
            let absorbed = env_val.min(config.absorption_rate);
            store.chem[i][k] = (store.chem[i][k] + absorbed).min(1.0);
            env.set(k, gx, gy, (env_val - absorbed).max(0.0));

            // Excretion: release excess
            if store.chem[i][k] > config.excretion_threshold {
                let excess = (store.chem[i][k] - config.excretion_threshold) * 0.5;
                store.chem[i][k] -= excess;
                env.add(k, gx, gy, excess);
            }
        }
    }

    // Field diffusion and decay
    env.diffuse(config.env_diffusion);
    env.decay(config.env_decay);
}
