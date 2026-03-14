use smallvec::SmallVec;

/// Number of distinct chemical channels in the simulation.
pub const NUM_CHEMICALS: usize = 8;

// ── Helper constants ────────────────────────────────────────────────────────
pub const DIFFUSION_RATE: f32 = 0.05;
pub const REACTION_RATE_MAX: f32 = 0.2;
pub const EXCRETION_THRESHOLD: f32 = 0.8;
pub const ABSORPTION_RATE: f32 = 0.02;
pub const ENV_DIFFUSION: f32 = 0.03;
pub const ENV_DECAY: f32 = 0.001;

/// A chemical reaction rule.
/// Substrates are consumed, products are created.
/// If an inhibitor is present and its concentration exceeds the threshold, the reaction is blocked.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Reaction {
    /// Substrates consumed: (chemical_index, amount)
    pub substrates: SmallVec<[(usize, f32); 2]>,
    /// Products created: (chemical_index, amount)
    pub products: SmallVec<[(usize, f32); 2]>,
    /// Reaction rate (0.0–1.0)
    pub rate: f32,
    /// Energy delta: positive = exothermic, negative = endothermic
    pub delta_energy: f32,
    /// Optional inhibitor: (chemical_index, threshold).
    /// If chem[idx] > threshold, reaction is blocked.
    pub inhibitor: Option<(usize, f32)>,
}

// ── Core chemistry functions ────────────────────────────────────────────────

/// Execute a single reaction on a chemical concentration vector.
///
/// 1. Check inhibitor — if present and concentration exceeds threshold, abort.
/// 2. Check all substrates are available (concentration >= amount * rate).
/// 3. Consume substrates, produce products (scaled by rate), adjust energy.
/// 4. Clamp all concentrations to [0.0, 1.0].
pub fn execute_reaction(
    chem: &mut [f32; NUM_CHEMICALS],
    energy: &mut f32,
    reaction: &Reaction,
) {
    // Check inhibitor
    if let Some((idx, threshold)) = reaction.inhibitor {
        if idx < NUM_CHEMICALS && chem[idx] > threshold {
            return;
        }
    }

    // Check substrate availability
    for &(idx, amount) in reaction.substrates.iter() {
        if idx >= NUM_CHEMICALS {
            return;
        }
        if chem[idx] < amount * reaction.rate {
            return;
        }
    }

    // Consume substrates
    for &(idx, amount) in reaction.substrates.iter() {
        chem[idx] -= amount * reaction.rate;
    }

    // Produce products
    for &(idx, amount) in reaction.products.iter() {
        if idx < NUM_CHEMICALS {
            chem[idx] += amount * reaction.rate;
        }
    }

    // Adjust energy
    *energy += reaction.delta_energy * reaction.rate;

    // Clamp concentrations
    for c in chem.iter_mut() {
        *c = c.clamp(0.0, 1.0);
    }
}

/// Dot-product-based affinity between two chemical signatures.
///
/// Returns `sum((a[k] - 0.5) * (b[k] - 0.5)) / N`, which lies in
/// approximately [-0.25, +0.25].
pub fn compute_affinity(
    chem_a: &[f32; NUM_CHEMICALS],
    chem_b: &[f32; NUM_CHEMICALS],
) -> f32 {
    let mut sum = 0.0_f32;
    for k in 0..NUM_CHEMICALS {
        sum += (chem_a[k] - 0.5) * (chem_b[k] - 0.5);
    }
    sum / NUM_CHEMICALS as f32
}

/// Bond strength based on complementarity.
///
/// `sum(min(a[k], 1.0 - b[k])) / N` — higher when chemicals are complementary.
pub fn compute_bond_strength(
    chem_a: &[f32; NUM_CHEMICALS],
    chem_b: &[f32; NUM_CHEMICALS],
) -> f32 {
    let mut sum = 0.0_f32;
    for k in 0..NUM_CHEMICALS {
        sum += chem_a[k].min(1.0 - chem_b[k]);
    }
    sum / NUM_CHEMICALS as f32
}

/// Diffuse chemicals between two concentration vectors.
///
/// For each channel, flux = (a[k] - b[k]) * rate. Transfer from higher to lower.
/// Clamp results to [0.0, 1.0].
pub fn diffuse_chemicals(
    chem_a: &mut [f32; NUM_CHEMICALS],
    chem_b: &mut [f32; NUM_CHEMICALS],
    rate: f32,
) {
    for k in 0..NUM_CHEMICALS {
        let flux = (chem_a[k] - chem_b[k]) * rate;
        chem_a[k] -= flux;
        chem_b[k] += flux;
        chem_a[k] = chem_a[k].clamp(0.0, 1.0);
        chem_b[k] = chem_b[k].clamp(0.0, 1.0);
    }
}

/// Derive an RGB color from the first three chemical concentrations.
pub fn particle_color(chem: &[f32; NUM_CHEMICALS]) -> [f32; 3] {
    [chem[0], chem[1], chem[2]]
}

/// Particle radius from the sum of all concentrations.
///
/// `0.3 + sum * 0.05`, clamped to [0.3, 0.7].
pub fn particle_radius(chem: &[f32; NUM_CHEMICALS]) -> f32 {
    let sum: f32 = chem.iter().sum();
    (0.3 + sum * 0.05).clamp(0.3, 0.7)
}

/// Motor force output — uses chemical channel 5.
pub fn motor_force(chem: &[f32; NUM_CHEMICALS]) -> f32 {
    chem[5]
}

/// Membrane rigidity — uses chemical channel 7.
pub fn membrane_rigidity(chem: &[f32; NUM_CHEMICALS]) -> f32 {
    chem[7]
}

/// Signal emission strength — uses chemical channel 3.
pub fn signal_strength(chem: &[f32; NUM_CHEMICALS]) -> f32 {
    chem[3]
}
