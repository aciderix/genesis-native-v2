use crate::chemistry::Reaction;
use crate::genome::ComposableGenome;
use std::collections::HashSet;

/// A snapshot of simulation metrics at a given tick.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MetricsSnapshot {
    pub tick: u64,
    pub population: usize,
    pub num_organisms: usize,
    pub num_species: usize,
    pub avg_ai: f32,
    pub max_ai: usize,
    pub innovation_rate: f32,
    pub phylo_diversity: f32,
    pub avg_genome_length: f32,
    pub max_genome_length: usize,
    pub total_energy: f64,
}

impl MetricsSnapshot {
    /// CSV header line.
    pub fn csv_header() -> &'static str {
        "tick,population,num_organisms,num_species,avg_ai,max_ai,innovation_rate,phylo_diversity,avg_genome_length,max_genome_length,total_energy"
    }

    /// Format this snapshot as a single CSV row.
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{:.4},{},{:.4},{:.4},{:.2},{},{:.2}",
            self.tick,
            self.population,
            self.num_organisms,
            self.num_species,
            self.avg_ai,
            self.max_ai,
            self.innovation_rate,
            self.phylo_diversity,
            self.avg_genome_length,
            self.max_genome_length,
            self.total_energy,
        )
    }
}

/// Compute the Assembly Index of a genome (delegates to `ComposableGenome::assembly_index`).
pub fn assembly_index(genome: &ComposableGenome) -> usize {
    genome.assembly_index()
}

/// Hash a reaction to a `u64` for identity comparison.
///
/// Uses FNV-1a-style manual hashing over substrate/product indices, quantized
/// amounts, and quantized rate.
pub fn reaction_hash(r: &Reaction) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    let prime: u64 = 0x100000001b3; // FNV prime

    for &(idx, amount) in r.substrates.iter() {
        h ^= idx as u64;
        h = h.wrapping_mul(prime);
        h ^= (amount * 1000.0) as u64;
        h = h.wrapping_mul(prime);
    }

    h ^= 0xFF;
    h = h.wrapping_mul(prime);

    for &(idx, amount) in r.products.iter() {
        h ^= idx as u64;
        h = h.wrapping_mul(prime);
        h ^= (amount * 1000.0) as u64;
        h = h.wrapping_mul(prime);
    }

    h ^= (r.rate * 10000.0) as u64;
    h = h.wrapping_mul(prime);

    h
}

/// Count new unique reactions across a set of genomes that were not seen before.
///
/// Returns `(new_count, updated_set)` where `updated_set` is `previous_reactions`
/// merged with all reactions found in `current_genomes`.
pub fn innovation_count(
    current_genomes: &[ComposableGenome],
    previous_reactions: &HashSet<u64>,
) -> (usize, HashSet<u64>) {
    let mut updated = previous_reactions.clone();
    let mut new_count = 0_usize;

    for genome in current_genomes {
        for reaction in &genome.reactions {
            let h = reaction_hash(reaction);
            if !previous_reactions.contains(&h) && updated.insert(h) {
                new_count += 1;
            }
        }
    }

    (new_count, updated)
}

/// Approximate phylogenetic diversity.
///
/// PD ≈ (number of unique lineages) × (average generation depth).
pub fn phylogenetic_diversity(parent_ids: &[i32], generations: &[u32]) -> f32 {
    if parent_ids.is_empty() || generations.is_empty() {
        return 0.0;
    }

    let unique_lineages: HashSet<i32> = parent_ids.iter().copied().collect();
    let num_lineages = unique_lineages.len() as f32;

    let avg_gen = if generations.is_empty() {
        0.0
    } else {
        generations.iter().map(|&g| g as f64).sum::<f64>() / generations.len() as f64
    };

    num_lineages * avg_gen as f32
}
