//! Bond formation and breaking systems.
//!
//! Ports the TypeScript `formBonds()` and `breakBonds()` methods.
//!
//! ## Bond Formation (`form_bonds_system`)
//!
//! For each alive, non-deposit particle with room for more bonds:
//! 1. Query spatial grid for nearby particles
//! 2. If within bond distance and probability check passes, form a bond
//! 3. Probability depends on: affinity, bond strength, temperature, catalyst
//!    proximity, combo bonus, gene expression, and epigenetic weight
//!
//! ## Bond Breaking (`break_bonds_system`)
//!
//! For each bonded pair:
//! 1. If distance > break threshold, break the bond
//! 2. If energy is very low, bonds become fragile
//! 3. Random thermal breaking (temperature-dependent)
//! 4. Defense role protects bonds from breaking
//! 5. Higher generation organisms have more stable bonds
//! 6. On break in organism: emit alarm pheromone, apply epi stress marks,
//!    handle predation energy theft, emit danger symbols

use bevy::prelude::*;
use crate::particle_store::{ParticleStore, SimRng};
use crate::config::SimConfig;
use crate::resources::*;
use crate::util::SpatialGrid;
use crate::components::{ParticleType, CellRole, MAX_BONDS};

// ---------------------------------------------------------------------------
// Constants — matching the TypeScript reference implementation
// ---------------------------------------------------------------------------

/// Base probability of bond formation per tick per eligible pair.
const BASE_BOND_PROB: f32 = 0.03;

/// Catalyst proximity bonus to bond formation probability.
const CATALYST_BOND_BONUS: f32 = 0.04;

/// Combo bonus multiplier for bond formation probability.
const COMBO_BOND_FACTOR: f32 = 0.02;

/// Gene expression bonus to bond formation probability.
const GENE_BOND_FACTOR: f32 = 0.02;

/// Epigenetic weight factor for bond formation.
const EPI_BOND_FACTOR: f32 = 0.01;

/// Distance multiplier for bond breaking threshold: `bond_distance * BREAK_DIST_MULT`.
const BREAK_DIST_MULT: f32 = 2.2;

/// Energy threshold below which bonds become fragile.
const FRAGILE_ENERGY: f32 = 0.5;

/// Base probability of random thermal bond breaking per tick.
const THERMAL_BREAK_BASE: f32 = 0.002;

/// Defense role bond protection factor (reduces break probability).
const DEFENSE_PROTECTION: f32 = 0.5;

/// Generational stability factor: each generation reduces break prob by this.
const GEN_STABILITY: f32 = 0.02;

/// Maximum generational stability reduction.
const MAX_GEN_STABILITY: f32 = 0.5;

/// Energy theft on predation break (fraction of victim's energy stolen).
const PREDATION_ENERGY_THEFT: f32 = 0.3;

/// Alarm pheromone emission amount on organism bond break.
const ALARM_EMISSION: f32 = 0.5;

/// Epigenetic stress mark intensity on bond break in organism.
const EPI_STRESS_MARK: f32 = 0.15;

// ---------------------------------------------------------------------------
// Bond Formation
// ---------------------------------------------------------------------------

/// Attempt to form new bonds between nearby particles.
///
/// For each alive, non-deposit particle with available bond slots:
/// 1. Query the spatial grid for neighbors within `bond_distance`
/// 2. Check if the neighbor also has room for bonds
/// 3. Compute formation probability based on:
///    - Base affinity between the two types
///    - Bond strength from the matrices
///    - Temperature (higher temp = more bonding attempts)
///    - Catalyst proximity (catalysts boost nearby bond formation)
///    - Combo bonus, gene expression, epigenetic weight
/// 4. Roll the dice and form the bond if probability check passes
pub fn form_bonds_inner(
    store: &mut ParticleStore,
    grid: &SpatialGrid,
    config: &SimConfig,
    matrices: &SimMatrices,
    rng: &mut SimRng,
) {
    let n = store.len();
    if n == 0 {
        return;
    }

    let ws = config.world_size;
    let bd = config.bond_distance;
    let bd_sq = bd * bd;

    // Collect bond formation candidates: (index_a, index_b)
    // We collect first, then form bonds to avoid aliasing issues.
    let mut new_bonds: Vec<(usize, usize)> = Vec::new();
    let mut neighbors_buf: Vec<usize> = Vec::with_capacity(128);

    for i in 0..n {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }
        if store.bonds[i].len() >= MAX_BONDS {
            continue; // No room for more bonds
        }

        grid.query_into(store.x[i], store.y[i], store.z[i], &mut neighbors_buf);

        for &j in &neighbors_buf {
            if j <= i || !store.alive[j] || store.is_deposit[j] {
                continue; // Only check each pair once (j > i), skip dead/deposit
            }
            if store.bonds[j].len() >= MAX_BONDS {
                continue; // Neighbor has no room
            }
            // Already bonded?
            if store.bonds[i].contains(&store.id[j]) {
                continue;
            }

            // Distance check
            let dist_sq = store.distance_sq_wrapped(i, j, ws);
            if dist_sq > bd_sq || dist_sq < 1e-10 {
                continue;
            }

            let ti = store.ptype[i].as_index();
            let tj = store.ptype[j].as_index();

            // --- Compute bond formation probability ---
            let affinity = matrices.affinity[ti][tj];
            let bond_str = matrices.bond_str[ti][tj];

            // Base probability scales with affinity and bond strength
            let mut prob = BASE_BOND_PROB * (affinity.max(0.0) + 0.1) * bond_str;

            // Temperature boost: higher temperature → more kinetic energy → more collisions
            prob += config.temperature * 0.01;

            // Catalyst proximity bonus: either particle has a catalyst bonded
            if store.has_catalyst(i) || store.has_catalyst(j) {
                prob += CATALYST_BOND_BONUS;
            }

            // Both particles are catalysts? Extra boost
            if store.ptype[i] == ParticleType::Catalyst
                && store.ptype[j] == ParticleType::Catalyst
            {
                prob += 0.02;
            }

            // Combo bonus: existing combos make further bonding easier
            let combo_avg = (store.combo_bonus[i] + store.combo_bonus[j]) * 0.5;
            prob += combo_avg * COMBO_BOND_FACTOR;

            // Gene expression bonus
            let gene_avg = (store.gene_expr[i] + store.gene_expr[j]) * 0.5;
            prob += gene_avg * GENE_BOND_FACTOR;

            // Epigenetic weight factor
            let epi_avg = (store.epi_weight[i] + store.epi_weight[j]) * 0.5;
            if epi_avg > 1.0 {
                prob += (epi_avg - 1.0) * EPI_BOND_FACTOR;
            }

            // Distance factor: closer particles bond more readily
            let dist = dist_sq.sqrt();
            let dist_factor = 1.0 - (dist / bd);
            prob *= dist_factor.max(0.1);

            // Roll the dice
            if rng.next() < prob.clamp(0.0, 0.5) {
                new_bonds.push((i, j));
            }
        }
    }

    // Apply the bond formations
    for (a, b) in new_bonds {
        // Re-check capacity (may have changed during this tick)
        if store.bonds[a].len() < MAX_BONDS && store.bonds[b].len() < MAX_BONDS {
            store.form_bond(a, b);
        }
    }
}

pub fn form_bonds_system(
    mut store: ResMut<ParticleStore>,
    grid: Res<SpatialGrid>,
    config: Res<SimConfig>,
    matrices: Res<SimMatrices>,
    mut rng: ResMut<SimRng>,
) {
    form_bonds_inner(&mut *store, &*grid, &*config, &*matrices, &mut *rng);
}

// ---------------------------------------------------------------------------
// Bond Breaking
// ---------------------------------------------------------------------------

/// Check all existing bonds and break those that exceed distance thresholds,
/// have insufficient energy, or fail random thermal stability checks.
///
/// ## Breaking Conditions
///
/// A bond breaks if ANY of these conditions are met:
/// 1. **Distance**: distance between bonded particles exceeds `bond_distance * BREAK_DIST_MULT`
/// 2. **Energy depletion**: either particle has energy < `FRAGILE_ENERGY` and random check fails
/// 3. **Thermal break**: random probability based on temperature (rare)
///
/// ## Protection Factors
///
/// - **Defense role**: Defense-role particles have bonds that are harder to break
/// - **Generational stability**: Higher-generation organisms have more stable bonds
///
/// ## Side Effects on Break
///
/// When a bond breaks within an organism:
/// - Alarm pheromone is emitted at the break location
/// - Epigenetic stress marks are applied to both particles
/// - If a Motor from a different organism is nearby (predation):
///   - Energy is transferred from victim to predator
///   - Predation counter is incremented
///   - Danger symbol is emitted
pub fn break_bonds_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
    matrices: &SimMatrices,
    counters: &mut SimCounters,
    events: &mut EventLog,
    fields: &mut SimFields,
    orgs: &OrganismRegistry,
    rng: &mut SimRng,
    stats: &SimStats,
) {
    let n = store.len();
    if n == 0 {
        return;
    }

    let ws = config.world_size;
    let bd = config.bond_distance;
    let break_dist = bd * BREAK_DIST_MULT;
    let break_dist_sq = break_dist * break_dist;

    // Collect bonds to break: (index_a, index_b)
    let mut to_break: Vec<(usize, usize)> = Vec::new();
    // Track predation events: (predator_idx, victim_idx)
    let mut predation_events: Vec<(usize, usize)> = Vec::new();

    // Iterate all particles and check their bonds
    for i in 0..n {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        // Collect bond partner IDs to iterate (avoid borrow issues)
        let partner_ids: Vec<u32> = store.bonds[i].iter().copied().collect();

        for partner_id in partner_ids {
            // Only process each pair once: i < j (by index)
            let j = match store.idx(partner_id) {
                Some(idx) if idx > i => idx,
                _ => continue,
            };

            if !store.alive[j] {
                // Partner died — mark for cleanup
                to_break.push((i, j));
                continue;
            }

            let dist_sq = store.distance_sq_wrapped(i, j, ws);
            let ti = store.ptype[i].as_index();
            let tj = store.ptype[j].as_index();
            let bond_str = matrices.bond_str[ti][tj];

            // --- Check breaking conditions ---

            let mut should_break = false;

            // 1. Distance exceeds break threshold
            if dist_sq > break_dist_sq {
                should_break = true;
            }

            // 2. Energy depletion makes bonds fragile
            if !should_break
                && (store.energy[i] < FRAGILE_ENERGY || store.energy[j] < FRAGILE_ENERGY)
            {
                let fragility = 1.0 - ((store.energy[i].min(store.energy[j])) / FRAGILE_ENERGY);
                if rng.next() < fragility * 0.1 {
                    should_break = true;
                }
            }

            // 3. Random thermal break
            if !should_break {
                let thermal_prob = THERMAL_BREAK_BASE * config.temperature / bond_str.max(0.1);

                // Defense role protection
                let mut protection = 1.0_f32;
                if store.cell_role[i] == CellRole::Defense
                    || store.cell_role[j] == CellRole::Defense
                {
                    protection *= DEFENSE_PROTECTION;
                }

                // Generational stability
                let my_org_id = store.organism_id[i];
                if my_org_id >= 0 {
                    if let Some(org) = orgs.get(my_org_id as u32) {
                        let gen_factor =
                            (org.generation as f32 * GEN_STABILITY).min(MAX_GEN_STABILITY);
                        protection *= 1.0 - gen_factor;
                    }
                }

                if rng.next() < thermal_prob * protection {
                    should_break = true;
                }
            }

            if should_break {
                to_break.push((i, j));

                // --- Side effects: bond break within an organism ---
                let org_i = store.organism_id[i];
                let org_j = store.organism_id[j];

                if org_i >= 0 && org_i == org_j {
                    // Emit alarm pheromone at the midpoint of the bond
                    let mid_x = (store.x[i] + store.x[j]) * 0.5;
                    let mid_y = (store.y[i] + store.y[j]) * 0.5;
                    let mid_z = (store.z[i] + store.z[j]) * 0.5;
                    fields.phero_alarm.inject(mid_x, mid_y, mid_z, ws, ALARM_EMISSION);

                    // Epigenetic stress marks
                    store.epi_weight[i] += EPI_STRESS_MARK;
                    store.epi_weight[j] += EPI_STRESS_MARK;

                    // Check for predation: is a Motor from a different organism nearby?
                    // (simplified check — look at the breaking particles' neighbors)
                    if store.ptype[i] == ParticleType::Motor && org_j >= 0 && org_i != org_j {
                        predation_events.push((i, j));
                    } else if store.ptype[j] == ParticleType::Motor
                        && org_i >= 0
                        && org_j != org_i
                    {
                        predation_events.push((j, i));
                    }
                }
            }
        }
    }

    // --- Apply bond breaks ---
    for &(a, b) in &to_break {
        store.break_bond(a, b);
    }

    // --- Process predation events ---
    for &(predator, victim) in &predation_events {
        if !store.alive[predator] || !store.alive[victim] {
            continue;
        }

        // Energy theft: predator steals a fraction of victim's energy
        let stolen = store.energy[victim] * PREDATION_ENERGY_THEFT;
        store.energy[victim] -= stolen;
        store.energy[predator] += stolen;

        // Increment global predation counter
        counters.total_pred += 1;

        // Emit danger symbol at victim location
        let sym = store.symbol_code[victim];
        if sym > 0 && (sym as usize) <= 8 {
            let ch = (sym - 1) as usize;
            fields.symbol[ch].inject(
                store.x[victim],
                store.y[victim],
                store.z[victim],
                ws,
                0.3,
            );
        }

        // Log the predation event
        events.push(
            stats.tick,
            format!(
                "Predation: Motor #{} stole {:.1} energy from #{} (V6.3)",
                store.id[predator],
                stolen,
                store.id[victim],
            ),
            EventType::Predation,
        );
    }

    // --- Log significant bond break events ---
    if to_break.len() > 10 {
        events.push(
            stats.tick,
            format!("{} bonds broken this tick", to_break.len()),
            EventType::Bond,
        );
    }
}

pub fn break_bonds_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    matrices: Res<SimMatrices>,
    mut counters: ResMut<SimCounters>,
    mut events: ResMut<EventLog>,
    mut fields: ResMut<SimFields>,
    orgs: Res<OrganismRegistry>,
    mut rng: ResMut<SimRng>,
    stats: Res<SimStats>,
) {
    break_bonds_inner(&mut *store, &*config, &*matrices, &mut *counters, &mut *events, &mut *fields, &*orgs, &mut *rng, &*stats);
}
