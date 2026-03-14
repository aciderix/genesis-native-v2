use bevy::prelude::*;
use crate::particle_store::ParticleStore;
use crate::resources::{GroupRegistry, MetricsHistory, SimTick};
use genesis_core::metrics::{innovation_count, reaction_hash, MetricsSnapshot};

/// Compute and store metrics every 50 ticks.
pub fn metrics_system(
    store: Res<ParticleStore>,
    tick: Res<SimTick>,
    mut history: ResMut<MetricsHistory>,
    groups: Res<GroupRegistry>,
) {
    // Only compute every 50 ticks
    if tick.0 % 50 != 0 {
        return;
    }

    let pop = store.population();
    if pop == 0 {
        return;
    }

    // Assembly index stats
    let mut total_ai = 0usize;
    let mut max_ai = 0usize;
    let mut total_genome_len = 0usize;
    let mut max_genome_len = 0usize;
    let mut total_energy = 0.0f64;

    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        let ai = store.genomes[i].assembly_index();
        total_ai += ai;
        if ai > max_ai {
            max_ai = ai;
        }
        let gl = store.genomes[i].reactions.len();
        total_genome_len += gl;
        if gl > max_genome_len {
            max_genome_len = gl;
        }
        total_energy += store.energy[i] as f64;
    }

    // Innovation count
    let genomes: Vec<_> = (0..store.count)
        .filter(|&i| store.alive[i])
        .map(|i| store.genomes[i].clone())
        .collect();
    let (innovation, updated_set) = innovation_count(&genomes, &history.known_reactions);
    history.known_reactions = updated_set;

    // Phylogenetic diversity approximation
    // Only pass data for alive particles
    let alive_parent_ids: Vec<i32> = (0..store.count)
        .filter(|&i| store.alive[i])
        .map(|i| store.parent_ids[i])
        .collect();
    let alive_generations: Vec<u32> = (0..store.count)
        .filter(|&i| store.alive[i])
        .map(|i| store.generations[i])
        .collect();
    let pd = genesis_core::metrics::phylogenetic_diversity(&alive_parent_ids, &alive_generations);

    let snapshot = MetricsSnapshot {
        tick: tick.0,
        population: pop,
        num_organisms: groups.groups.len(),
        num_species: estimate_species(&store),
        avg_ai: if pop > 0 {
            total_ai as f32 / pop as f32
        } else {
            0.0
        },
        max_ai,
        innovation_rate: innovation as f32,
        phylo_diversity: pd,
        avg_genome_length: if pop > 0 {
            total_genome_len as f32 / pop as f32
        } else {
            0.0
        },
        max_genome_length: max_genome_len,
        total_energy,
    };
    history.snapshots.push(snapshot);
}

fn estimate_species(store: &ParticleStore) -> usize {
    // Simple species estimation: count distinct genome reaction patterns (first 3 reactions hash)
    let mut species_set = std::collections::HashSet::new();
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        let mut hash = 0u64;
        for (idx, r) in store.genomes[i].reactions.iter().enumerate().take(3) {
            hash ^= reaction_hash(r).wrapping_mul(idx as u64 + 1);
        }
        species_set.insert(hash);
    }
    species_set.len()
}
