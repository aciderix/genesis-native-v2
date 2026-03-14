use crate::chemistry::{Reaction, REACTION_RATE_MAX, NUM_CHEMICALS};
use smallvec::SmallVec;
use std::collections::HashSet;

/// A composable genome: variable-length sequence of chemical reactions.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ComposableGenome {
    /// The reactions encoded by this genome.
    pub reactions: Vec<Reaction>,
    /// Initial chemical concentrations for offspring.
    pub initial_chem: [f32; NUM_CHEMICALS],
}

// ── Helper: create a random reaction ────────────────────────────────────────

fn random_reaction(rng: &mut dyn FnMut() -> f32) -> Reaction {
    let sub_count = if rng() < 0.5 { 1 } else { 2 };
    let prod_count = if rng() < 0.5 { 1 } else { 2 };

    let mut substrates = SmallVec::new();
    for _ in 0..sub_count {
        let idx = (rng() * NUM_CHEMICALS as f32) as usize % NUM_CHEMICALS;
        let amount = rng() * 0.3 + 0.05;
        substrates.push((idx, amount));
    }

    let mut products = SmallVec::new();
    for _ in 0..prod_count {
        let idx = (rng() * NUM_CHEMICALS as f32) as usize % NUM_CHEMICALS;
        let amount = rng() * 0.3 + 0.05;
        products.push((idx, amount));
    }

    let rate = rng() * REACTION_RATE_MAX;
    let delta_energy = (rng() - 0.5) * 0.1; // [-0.05, +0.05]

    let inhibitor = if rng() < 0.2 {
        let idx = (rng() * NUM_CHEMICALS as f32) as usize % NUM_CHEMICALS;
        Some((idx, rng() * 0.5 + 0.3))
    } else {
        None
    };

    Reaction {
        substrates,
        products,
        rate,
        delta_energy,
        inhibitor,
    }
}

impl ComposableGenome {
    /// Create a random genome with 3 reactions.
    pub fn random(rng: &mut dyn FnMut() -> f32) -> Self {
        let reactions: Vec<Reaction> = (0..3).map(|_| random_reaction(rng)).collect();

        let mut initial_chem = [0.0_f32; NUM_CHEMICALS];
        for c in initial_chem.iter_mut() {
            *c = rng();
        }

        ComposableGenome {
            reactions,
            initial_chem,
        }
    }

    /// Apply mutations to this genome.
    ///
    /// Mutation operators:
    /// - **Point mutation** (5% per reaction): modify a substrate idx, product idx, rate, or delta_energy.
    /// - **Insertion** (2%): add a new random reaction.
    /// - **Deletion** (2%): remove a random reaction (keep at least 1).
    /// - **Duplication** (1%): copy a random existing reaction.
    /// - **Inversion** (1%): swap substrates and products of a random reaction.
    /// - **Transposition** (1%): move a reaction to a different position.
    pub fn mutate(&mut self, rng: &mut dyn FnMut() -> f32) {
        // Point mutations — 5% chance per reaction
        for i in 0..self.reactions.len() {
            if rng() < 0.05 {
                // Pick what to mutate: 0=substrate idx, 1=product idx, 2=rate, 3=delta_energy
                let choice = (rng() * 4.0) as u32;
                match choice {
                    0 => {
                        // Mutate a substrate index
                        if !self.reactions[i].substrates.is_empty() {
                            let si = (rng() * self.reactions[i].substrates.len() as f32) as usize
                                % self.reactions[i].substrates.len();
                            self.reactions[i].substrates[si].0 =
                                (rng() * NUM_CHEMICALS as f32) as usize % NUM_CHEMICALS;
                        }
                    }
                    1 => {
                        // Mutate a product index
                        if !self.reactions[i].products.is_empty() {
                            let pi = (rng() * self.reactions[i].products.len() as f32) as usize
                                % self.reactions[i].products.len();
                            self.reactions[i].products[pi].0 =
                                (rng() * NUM_CHEMICALS as f32) as usize % NUM_CHEMICALS;
                        }
                    }
                    2 => {
                        // Mutate rate
                        self.reactions[i].rate =
                            (self.reactions[i].rate + (rng() - 0.5) * 0.05).clamp(0.0, REACTION_RATE_MAX);
                    }
                    _ => {
                        // Mutate delta_energy
                        self.reactions[i].delta_energy += (rng() - 0.5) * 0.02;
                    }
                }
            }
        }

        // Insertion — 2%
        if rng() < 0.02 {
            self.reactions.push(random_reaction(rng));
        }

        // Deletion — 2% (keep at least 1)
        if rng() < 0.02 && self.reactions.len() > 1 {
            let idx = (rng() * self.reactions.len() as f32) as usize % self.reactions.len();
            self.reactions.remove(idx);
        }

        // Duplication — 1%
        if rng() < 0.01 && !self.reactions.is_empty() {
            let idx = (rng() * self.reactions.len() as f32) as usize % self.reactions.len();
            let copy = self.reactions[idx].clone();
            self.reactions.push(copy);
        }

        // Inversion — 1%: swap substrates and products of a random reaction
        if rng() < 0.01 && !self.reactions.is_empty() {
            let idx = (rng() * self.reactions.len() as f32) as usize % self.reactions.len();
            let r = &mut self.reactions[idx];
            std::mem::swap(&mut r.substrates, &mut r.products);
        }

        // Transposition — 1%: move a reaction to a different position
        if rng() < 0.01 && self.reactions.len() > 1 {
            let from = (rng() * self.reactions.len() as f32) as usize % self.reactions.len();
            let to = (rng() * self.reactions.len() as f32) as usize % self.reactions.len();
            if from != to {
                let r = self.reactions.remove(from);
                let insert_at = to.min(self.reactions.len());
                self.reactions.insert(insert_at, r);
            }
        }

        // Also slightly mutate initial_chem (5% per channel)
        for c in self.initial_chem.iter_mut() {
            if rng() < 0.05 {
                *c = (*c + (rng() - 0.5) * 0.1).clamp(0.0, 1.0);
            }
        }
    }

    /// Approximate Assembly Index using LZ-like decomposition.
    ///
    /// Hash each reaction to a `u64`, then count the number of unique
    /// substrings (fragments) needed to reconstruct the sequence.
    pub fn assembly_index(&self) -> usize {
        if self.reactions.is_empty() {
            return 0;
        }

        // Hash each reaction to a u64 token
        let tokens: Vec<u64> = self.reactions.iter().map(|r| hash_reaction(r)).collect();

        // LZ-like decomposition: greedily find longest previously-seen prefix
        let mut dictionary: HashSet<Vec<u64>> = HashSet::new();
        let mut idx = 0;
        let mut fragment_count = 0;

        while idx < tokens.len() {
            let mut length = 1;
            // Extend the fragment as long as it's already in the dictionary
            while idx + length <= tokens.len()
                && dictionary.contains(&tokens[idx..idx + length])
            {
                length += 1;
            }
            // The fragment tokens[idx..idx+length] is new — add it
            dictionary.insert(tokens[idx..idx + length].to_vec());
            fragment_count += 1;
            idx += length;
        }

        fragment_count
    }
}

/// Crossover: cut each parent at a random point, combine first half of A
/// with second half of B. Mix `initial_chem` by averaging.
pub fn crossover(
    parent_a: &ComposableGenome,
    parent_b: &ComposableGenome,
    rng: &mut dyn FnMut() -> f32,
) -> ComposableGenome {
    let cut_a = if parent_a.reactions.is_empty() {
        0
    } else {
        (rng() * parent_a.reactions.len() as f32) as usize % parent_a.reactions.len()
    };
    let cut_b = if parent_b.reactions.is_empty() {
        0
    } else {
        (rng() * parent_b.reactions.len() as f32) as usize % parent_b.reactions.len()
    };

    let mut reactions = Vec::new();
    // First half of parent A (up to cut_a)
    for r in parent_a.reactions.iter().take(cut_a) {
        reactions.push(r.clone());
    }
    // Second half of parent B (from cut_b onward)
    for r in parent_b.reactions.iter().skip(cut_b) {
        reactions.push(r.clone());
    }

    // Ensure at least one reaction
    if reactions.is_empty() {
        if !parent_a.reactions.is_empty() {
            reactions.push(parent_a.reactions[0].clone());
        } else if !parent_b.reactions.is_empty() {
            reactions.push(parent_b.reactions[0].clone());
        } else {
            reactions.push(random_reaction(rng));
        }
    }

    // Average initial_chem
    let mut initial_chem = [0.0_f32; NUM_CHEMICALS];
    for k in 0..NUM_CHEMICALS {
        initial_chem[k] = (parent_a.initial_chem[k] + parent_b.initial_chem[k]) * 0.5;
    }

    ComposableGenome {
        reactions,
        initial_chem,
    }
}

/// Hash a reaction to a u64 for comparison / assembly-index computation.
fn hash_reaction(r: &Reaction) -> u64 {
    // Simple manual hash combining substrate indices, product indices, and quantized rate
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    let prime: u64 = 0x100000001b3; // FNV prime

    for &(idx, amount) in r.substrates.iter() {
        h ^= idx as u64;
        h = h.wrapping_mul(prime);
        h ^= (amount * 1000.0) as u64;
        h = h.wrapping_mul(prime);
    }

    // Separator between substrates and products
    h ^= 0xFF;
    h = h.wrapping_mul(prime);

    for &(idx, amount) in r.products.iter() {
        h ^= idx as u64;
        h = h.wrapping_mul(prime);
        h ^= (amount * 1000.0) as u64;
        h = h.wrapping_mul(prime);
    }

    // Quantized rate
    h ^= (r.rate * 10000.0) as u64;
    h = h.wrapping_mul(prime);

    h
}
