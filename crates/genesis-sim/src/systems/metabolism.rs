use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;

/// Energy-based metabolism: base cost + genome complexity + age + death.
pub fn metabolism_system(mut store: ResMut<ParticleStore>, config: Res<SimConfig>) {
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }

        // Base metabolism cost
        store.energy[i] -= config.base_metabolism;

        // Genome complexity cost
        let genome_len = store.genomes[i].reactions.len() as f32;
        store.energy[i] -= genome_len * 0.0001;

        // Age increment
        store.ages[i] += 1;

        // Death check
        if store.energy[i] <= config.death_energy_threshold {
            store.alive[i] = false;
        }
    }

    // Periodic compaction: remove dead particles
    let any_dead = store.alive.iter().any(|&a| !a);
    if any_dead {
        store.compact();
    }
}
