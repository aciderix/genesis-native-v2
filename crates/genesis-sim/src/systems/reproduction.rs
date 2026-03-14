use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::SimRng;
use genesis_core::chemistry::NUM_CHEMICALS;
use genesis_core::genome::{crossover, ComposableGenome};

/// Composable genome crossover and mutation.
pub fn reproduction_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut rng: ResMut<SimRng>,
) {
    if store.count >= config.max_particles {
        return;
    }

    let count = store.count;
    let mut offspring: Vec<(
        f32,
        f32,
        [f32; NUM_CHEMICALS],
        ComposableGenome,
        f32,
        i32,
        u32,
    )> = Vec::new();

    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        if store.energy[i] < config.reproduction_energy_threshold {
            continue;
        }
        if store.ages[i] < config.min_reproduction_age {
            continue;
        }
        if offspring.len() + store.count >= config.max_particles {
            break;
        }

        // Find a bonded partner for sexual reproduction, or reproduce asexually
        let child_genome = if !store.bonds[i].is_empty() {
            let bond_idx = rng.next_usize(store.bonds[i].len());
            let (partner, _) = store.bonds[i][bond_idx];
            if partner < count
                && store.alive[partner]
                && store.energy[partner] > config.reproduction_cost * 0.5
            {
                // Sexual reproduction: crossover
                let mut rng_fn = || rng.next_f32();
                crossover(&store.genomes[i], &store.genomes[partner], &mut rng_fn)
            } else {
                // Asexual: clone
                store.genomes[i].clone()
            }
        } else {
            // Asexual: clone
            store.genomes[i].clone()
        };

        // Mutate
        let mut child_genome = child_genome;
        let mut rng_fn = || rng.next_f32();
        child_genome.mutate(&mut rng_fn);

        // Offspring position: nearby parent
        let ox = store.x[i] + rng.range(-10.0, 10.0);
        let oy = store.y[i] + rng.range(-10.0, 10.0);

        // Energy transfer
        store.energy[i] -= config.reproduction_cost;
        let child_energy = config.reproduction_cost * 0.5;

        let parent_id = store.particle_ids[i] as i32;
        let generation = store.generations[i] + 1;

        offspring.push((
            ox,
            oy,
            child_genome.initial_chem,
            child_genome,
            child_energy,
            parent_id,
            generation,
        ));
    }

    // Add offspring
    for (x, y, chem, genome, energy, parent_id, generation) in offspring {
        store.add_particle(x, y, chem, genome, energy, parent_id, generation);
    }
}
