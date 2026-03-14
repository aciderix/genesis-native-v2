use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use genesis_core::chemistry::execute_reaction;

/// Internal chemistry: execute all genome reactions for each particle.
pub fn chemistry_system(mut store: ResMut<ParticleStore>, config: Res<SimConfig>) {
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        // Clone genome reactions to avoid borrow issues
        let reactions = store.genomes[i].reactions.clone();
        let mut chem = store.chem[i];
        let mut energy = store.energy[i];
        for reaction in &reactions {
            execute_reaction(&mut chem, &mut energy, reaction);
        }
        // Clamp energy
        energy = energy.clamp(0.0, config.max_energy);
        store.chem[i] = chem;
        store.energy[i] = energy;
    }
}
