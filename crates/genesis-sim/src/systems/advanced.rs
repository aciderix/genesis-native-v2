// ── advanced.rs ── Combos, Gene Regulation, Epigenetics, Cell Roles ─────────
//
// This module ports four subsystems from the TypeScript reference:
//   P2.1 — Combo Detection:   detect multi-type organism combos
//   P2.4 — Gene Regulation:   Data particles regulate bonded neighbors
//   P2.5 — Epigenetics:       environmental modification of epi_weight
//   P3.1 — Cell Roles:        assign specialized roles to organism members
//   P3.2 — Multicellularity:  detect organisms with diverse role composition

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::ParticleStore;

use std::collections::{HashMap, HashSet};

// ─── Combo Definitions ──────────────────────────────────────────────────────
// Each combo specifies: (name, required types, combo_id, tier, energy_bonus)

struct ComboDef {
    #[allow(dead_code)]
    name: &'static str,
    required: &'static [ParticleType],
    combo_id: u8,
    #[allow(dead_code)]
    tier: u8,
    energy_bonus: f32,
}

const COMBO_DEFS: &[ComboDef] = &[
    ComboDef {
        name: "photomotor",
        required: &[ParticleType::Catalyst, ParticleType::Beta, ParticleType::Motor],
        combo_id: 1,
        tier: 1,
        energy_bonus: 0.3,
    },
    ComboDef {
        name: "armored scout",
        required: &[ParticleType::Motor, ParticleType::Membrane, ParticleType::Catalyst],
        combo_id: 2,
        tier: 1,
        energy_bonus: 0.25,
    },
    ComboDef {
        name: "smart solar",
        required: &[ParticleType::Data, ParticleType::Catalyst, ParticleType::Beta],
        combo_id: 3,
        tier: 1,
        energy_bonus: 0.4,
    },
    ComboDef {
        name: "neural defender",
        required: &[ParticleType::Data, ParticleType::Motor, ParticleType::Membrane],
        combo_id: 4,
        tier: 1,
        energy_bonus: 0.35,
    },
    ComboDef {
        name: "full spectrum",
        required: &[
            ParticleType::Alpha,
            ParticleType::Beta,
            ParticleType::Catalyst,
            ParticleType::Motor,
        ],
        combo_id: 5,
        tier: 2,
        energy_bonus: 0.5,
    },
    ComboDef {
        name: "commander",
        required: &[
            ParticleType::Data,
            ParticleType::Catalyst,
            ParticleType::Motor,
            ParticleType::Membrane,
        ],
        combo_id: 6,
        tier: 2,
        energy_bonus: 0.6,
    },
];

// ─── P2.1: Combo Detection ─────────────────────────────────────────────────
//
// Every 10 ticks, scan organisms for specific type compositions.  When an
// organism contains at least one particle of every type in a combo definition,
// the combo is detected and a per-tick energy bonus is applied to all members.

pub fn combos_system(
    store: &mut ParticleStore,
    org_reg: &OrganismRegistry,
    counters: &SimCounters,
) {
    // Only run every 10 ticks
    if counters.tick % 10 != 0 {
        return;
    }

    for (_oid, oinfo) in &org_reg.organisms {
        // Count how many of each type the organism has
        let mut type_counts = [0u32; NUM_TYPES];
        let member_indices: Vec<usize> = oinfo
            .members
            .iter()
            .filter_map(|&pid| {
                let idx = *store.id_to_index.get(&pid)?;
                if store.alive[idx] && !store.is_deposit[idx] {
                    type_counts[store.ptype[idx].as_index()] += 1;
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if member_indices.is_empty() {
            continue;
        }

        // Find the best (highest bonus) matching combo
        let mut best_bonus: f32 = 0.0;
        let mut best_combo_id: u8 = 0;

        for combo in COMBO_DEFS {
            // Check if org has ≥1 of each required type
            let has_all = combo
                .required
                .iter()
                .all(|pt| type_counts[pt.as_index()] >= 1);

            if has_all && combo.energy_bonus > best_bonus {
                best_bonus = combo.energy_bonus;
                best_combo_id = combo.combo_id;
            }
        }

        // Apply combo bonus to all org particles
        if best_combo_id > 0 {
            for &idx in &member_indices {
                store.combo_bonus[idx] = best_bonus;
            }
        } else {
            // Clear combo bonus if no combo detected
            for &idx in &member_indices {
                store.combo_bonus[idx] = 0.0;
            }
        }
    }
}

// ─── P2.4: Gene Regulation ──────────────────────────────────────────────────
//
// Data particles act as transcription factors, regulating bonded non-Data
// particles within their organism.  Positive signal (>0.3) activates gene
// expression; negative signal (<-0.3) suppresses it.  When gene_expr > 0.5,
// type-specific bonuses are applied.

pub fn gene_regulation_system(
    store: &mut ParticleStore,
    org_reg: &OrganismRegistry,
) -> u32 {
    let mut active_gene_count: u32 = 0;
    let len = store.id.len();

    // First pass: collect regulation actions to avoid aliasing issues.
    // Each action: (target_index, delta_gene_expr)
    let mut actions: Vec<(usize, f32)> = Vec::new();

    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        // Only Data particles regulate
        if store.ptype[i] != ParticleType::Data {
            continue;
        }

        // Must be in an organism
        if store.organism_id[i] < 0 {
            continue;
        }

        let org_id = store.organism_id[i];
        let signal = store.signal[i];

        // Iterate bonded particles
        let bond_ids: Vec<u32> = store.bonds[i].iter().copied().collect();
        for bid in bond_ids {
            if let Some(&bi) = store.id_to_index.get(&bid) {
                if !store.alive[bi] || store.is_deposit[bi] {
                    continue;
                }
                // Must be in the same organism
                if store.organism_id[bi] != org_id {
                    continue;
                }
                // Must be a different type (Data doesn't regulate Data)
                if store.ptype[bi] == ParticleType::Data {
                    continue;
                }

                // Activation: signal > 0.3
                if signal > 0.3 {
                    actions.push((bi, 0.02 * signal));
                }
                // Suppression: signal < -0.3
                else if signal < -0.3 {
                    actions.push((bi, -0.01 * signal.abs()));
                }
            }
        }
    }

    // Apply gene expression changes
    for (idx, delta) in &actions {
        store.gene_expr[*idx] = (store.gene_expr[*idx] + delta).clamp(0.0, 1.0);
    }

    // Second pass: apply bonuses for expressed genes and count active genes
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        if store.gene_expr[i] > 0.5 {
            active_gene_count += 1;

            // Type-specific gene expression bonuses
            match store.ptype[i] {
                ParticleType::Alpha => {
                    // Alpha: direct energy bonus
                    store.energy[i] += 0.002;
                }
                ParticleType::Beta => {
                    // Beta: enhanced solar absorption (modeled as small energy bump)
                    // The actual solar*1.1 modifier is applied in metabolism
                    store.energy[i] += 0.001;
                }
                ParticleType::Catalyst => {
                    // Catalyst: detection range +10% — modeled as slight signal boost
                    store.signal[i] = (store.signal[i] + 0.005).clamp(-1.0, 1.0);
                }
                ParticleType::Motor => {
                    // Motor: speed +20% — apply velocity boost
                    store.vx[i] *= 1.002;
                    store.vy[i] *= 1.002;
                    store.vz[i] *= 1.002;
                }
                ParticleType::Membrane => {
                    // Membrane: defense +15% — modeled as energy resilience
                    store.energy[i] += 0.001;
                }
                ParticleType::Data => {
                    // Data doesn't get regulated but can self-express
                }
            }
        }
    }

    active_gene_count
}

// ─── P2.5: Epigenetics ──────────────────────────────────────────────────────
//
// Environmental conditions modify epi_weight:
//   • High energy (>8): epi_weight += 0.002  (thriving environment)
//   • Low energy (<2):  epi_weight -= 0.003  (stress response)
//   • In organism:       drift toward org average * 0.001
//   • Clamp to [-2, 2]

pub fn epigenetics_system(
    store: &mut ParticleStore,
    org_reg: &OrganismRegistry,
) {
    // Pre-compute average epi_weight per organism
    let mut org_epi_sums: HashMap<u32, (f32, u32)> = HashMap::new();

    let len = store.id.len();
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }
        if store.organism_id[i] >= 0 {
            let oid = store.organism_id[i] as u32;
            let entry = org_epi_sums.entry(oid).or_insert((0.0, 0));
            entry.0 += store.epi_weight[i];
            entry.1 += 1;
        }
    }

    let org_epi_avgs: HashMap<u32, f32> = org_epi_sums
        .iter()
        .map(|(&oid, &(sum, count))| {
            let avg = if count > 0 { sum / count as f32 } else { 1.0 };
            (oid, avg)
        })
        .collect();

    // Apply environmental epigenetic modifications
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        let energy = store.energy[i];

        // High energy environment: positive epigenetic drift
        if energy > 8.0 {
            store.epi_weight[i] += 0.002;
        }
        // Low energy environment: negative epigenetic drift (stress)
        else if energy < 2.0 {
            store.epi_weight[i] -= 0.003;
        }

        // Within an organism: drift toward the organism's average
        if store.organism_id[i] >= 0 {
            let oid = store.organism_id[i] as u32;
            if let Some(&avg) = org_epi_avgs.get(&oid) {
                let diff = avg - store.epi_weight[i];
                store.epi_weight[i] += diff * 0.001;
            }
        }

        // Clamp epi_weight to valid range
        store.epi_weight[i] = store.epi_weight[i].clamp(-2.0, 2.0);
    }
}

// ─── P3.1 / P3.2: Cell Roles & Multicellularity ────────────────────────────
//
// For organisms with 5+ particles:
//   1. Assign roles based on particle type + neighbor composition
//   2. Track per-role membership in org.cells
//   3. Detect multicellularity (2+ distinct roles with 2+ members each)
//   4. Compute specialization = normalized Shannon entropy of role distribution

pub fn cell_roles_system(
    store: &mut ParticleStore,
    org_reg: &mut OrganismRegistry,
) {
    let org_ids: Vec<u32> = org_reg.organisms.keys().copied().collect();

    for oid in org_ids {
        // Collect alive, non-deposit members
        let member_indices: Vec<usize> = {
            let oinfo = match org_reg.organisms.get(&oid) {
                Some(o) => o,
                None => continue,
            };
            oinfo
                .members
                .iter()
                .filter_map(|&pid| {
                    let idx = *store.id_to_index.get(&pid)?;
                    if store.alive[idx] && !store.is_deposit[idx] {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect()
        };

        let size = member_indices.len();

        // Only assign roles for organisms with 5+ particles
        if size < 5 {
            // Reset roles for small organisms
            for &idx in &member_indices {
                store.cell_role[idx] = CellRole::None;
            }
            if let Some(oinfo) = org_reg.organisms.get_mut(&oid) {
                oinfo.cells.clear();
                oinfo.is_multicellular = false;
                oinfo.specialization = 0.0;
            }
            continue;
        }

        // Count neighbor types for each particle (within the organism)
        // Build adjacency: which org members are bonded to this particle?
        let mut neighbor_type_counts: Vec<[u32; NUM_TYPES]> = vec![[0u32; NUM_TYPES]; size];

        // Map particle index → local index within this organism
        let mut idx_to_local: HashMap<usize, usize> = HashMap::new();
        for (local, &idx) in member_indices.iter().enumerate() {
            idx_to_local.insert(idx, local);
        }

        for (local, &idx) in member_indices.iter().enumerate() {
            for &bid in &store.bonds[idx] {
                if let Some(&bi) = store.id_to_index.get(&bid) {
                    if idx_to_local.contains_key(&bi) {
                        let bt = store.ptype[bi].as_index();
                        neighbor_type_counts[local][bt] += 1;
                    }
                }
            }
        }

        // Assign roles based on particle type + neighborhood
        let mut role_counts: HashMap<u8, HashSet<u32>> = HashMap::new();

        for (local, &idx) in member_indices.iter().enumerate() {
            let ptype = store.ptype[idx];
            let pid = store.id[idx];

            let role = match ptype {
                ParticleType::Catalyst => CellRole::Sensor,
                ParticleType::Beta => CellRole::Digester,
                ParticleType::Motor => CellRole::MotorCell,
                ParticleType::Membrane => CellRole::Defense,
                ParticleType::Data => CellRole::Reproducer,
                ParticleType::Alpha => {
                    // Alpha: adopt the role of the most common neighbor type
                    let counts = &neighbor_type_counts[local];
                    let most_common = counts
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, &c)| c)
                        .map(|(t, _)| t)
                        .unwrap_or(0);

                    match ParticleType::from_index(most_common) {
                        ParticleType::Catalyst => CellRole::Sensor,
                        ParticleType::Beta => CellRole::Digester,
                        ParticleType::Motor => CellRole::MotorCell,
                        ParticleType::Membrane => CellRole::Defense,
                        ParticleType::Data => CellRole::Reproducer,
                        ParticleType::Alpha => CellRole::None,
                    }
                }
            };

            store.cell_role[idx] = role;
            role_counts
                .entry(role.as_index() as u8)
                .or_default()
                .insert(pid);
        }

        // Update organism info
        if let Some(oinfo) = org_reg.organisms.get_mut(&oid) {
            oinfo.cells = role_counts.clone();

            // Multicellular check: 2+ distinct roles with 2+ particles each
            let qualifying_roles = role_counts
                .iter()
                .filter(|(role_idx, members)| **role_idx > 0 && members.len() >= 2)
                .count();
            oinfo.is_multicellular = qualifying_roles >= 2;

            // Compute specialization as normalized Shannon entropy
            // Higher entropy = more evenly distributed roles = higher specialization
            let total_assigned: f32 = role_counts
                .iter()
                .filter(|(r, _)| **r > 0)
                .map(|(_, m)| m.len() as f32)
                .sum();

            if total_assigned > 0.0 {
                let num_active_roles = role_counts
                    .iter()
                    .filter(|(r, m)| **r > 0 && !m.is_empty())
                    .count();

                if num_active_roles > 1 {
                    let mut entropy: f32 = 0.0;
                    for (role_idx, members) in &role_counts {
                        if *role_idx == 0 || members.is_empty() {
                            continue;
                        }
                        let p = members.len() as f32 / total_assigned;
                        if p > 0.0 {
                            entropy -= p * p.ln();
                        }
                    }
                    // Normalize by max entropy (uniform distribution)
                    let max_entropy = (num_active_roles as f32).ln();
                    oinfo.specialization = if max_entropy > 0.0 {
                        (entropy / max_entropy).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                } else {
                    oinfo.specialization = 0.0;
                }
            } else {
                oinfo.specialization = 0.0;
            }
        }
    }
}

// ─── Combined Bevy system ───────────────────────────────────────────────────

/// Runs all advanced subsystems in sequence: combos → gene regulation →
/// epigenetics → cell roles & multicellularity.
pub fn advanced_systems_inner(
    store: &mut ParticleStore,
    org_reg: &mut OrganismRegistry,
    counters: &SimCounters,
    active_gene_count: &mut ActiveGeneCount,
) {
    combos_system(&mut store, &org_reg, &counters);
    let gene_count = gene_regulation_system(&mut store, &org_reg);
    active_gene_count.0 = gene_count;
    epigenetics_system(&mut store, &org_reg);
    cell_roles_system(&mut store, &mut org_reg);
}

pub fn advanced_systems(
    mut store: ResMut<ParticleStore>,
    mut org_reg: ResMut<OrganismRegistry>,
    counters: Res<SimCounters>,
    mut active_gene_count: ResMut<ActiveGeneCount>,
) {
    advanced_systems_inner(&mut *store, &mut *org_reg, &*counters, &mut *active_gene_count);
}
