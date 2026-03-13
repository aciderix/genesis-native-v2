// ── culture_metacog.rs ── Cultural Memes & Meta-Cognition ───────────────────
//
// This module ports two high-level emergent behavior systems:
//   P4.3 — Cultural Memes:   innovation, transmission, and behavioral effects
//   P4.4 — Meta-Cognition:   Data→Data chains enabling self-modulation

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::{ParticleStore, SimRng};

use std::collections::{HashMap, HashSet, VecDeque};

// ─── P4.3: Cultural Meme System ─────────────────────────────────────────────
//
// Every 10 ticks, for organisms with >5 particles:
//
// **Innovation**: organisms without a meme can spontaneously create one with
//   probability 0.003.  The meme is categorized by environmental context:
//   - Near predators → defensive meme (9-12)
//   - High fitness   → social meme (5-8)
//   - Near vent      → tool/build meme (13-16)
//   - Default        → foraging meme (1-4)
//
// **Transmission**: organisms with memes can transmit to nearby organisms.
//   Probability boosted by fitness gap, shared colony, symbols, metacognition.
//
// **Behavioral effects**: memes modify particle behavior per category.

pub fn culture_inner(
    store: &mut ParticleStore,
    org_reg: &mut OrganismRegistry,
    config: &SimConfig,
    counters: &SimCounters,
    events: &mut EventLog,
    cultural_count: &mut CulturalEventCount,
    fields: &mut SimFields,
    rng: &mut SimRng,
) {
    let tick = counters.tick;
    let ws = config.world_size;
    let ir = config.interaction_radius;

    // Only run every 10 ticks
    if tick % 10 != 0 {
        // Still apply meme behavioral effects every tick
        apply_meme_effects(&mut store);
        return;
    }

    cultural_count.0 = 0;

    // Collect org info for iteration (avoids borrow conflicts)
    let org_snapshot: Vec<(u32, Vec<usize>, f32, u32, f32, i32, bool, u16)> = org_reg
        .organisms
        .iter()
        .filter_map(|(&oid, oinfo)| {
            let member_indices: Vec<usize> = oinfo
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
                .collect();

            if member_indices.len() <= 5 {
                return None;
            }

            // Compute centroid
            let (mut cx, mut cy, mut cz) = (0.0f32, 0.0f32, 0.0f32);
            for &idx in &member_indices {
                cx += store.x[idx];
                cy += store.y[idx];
                cz += store.z[idx];
            }
            let n = member_indices.len() as f32;
            cx /= n;
            cy /= n;
            cz /= n;

            // Check current meme from first particle
            let current_meme = if let Some(&first) = member_indices.first() {
                store.cultural_meme[first]
            } else {
                0
            };

            // Pack centroid into fitness (we'll use x/y/z separately below)
            // Actually, let's store the centroid differently
            Some((
                oid,
                member_indices,
                oinfo.fitness,
                oinfo.predation_count,
                oinfo.energy,
                oinfo.colony_id,
                oinfo.meta_cog_depth > 0.0,
                current_meme,
            ))
        })
        .collect();

    // Build centroid map for proximity checks
    let mut org_centroids: HashMap<u32, (f32, f32, f32)> = HashMap::new();
    for (oid, members, ..) in &org_snapshot {
        if members.is_empty() {
            continue;
        }
        let n = members.len() as f32;
        let cx: f32 = members.iter().map(|&i| store.x[i]).sum::<f32>() / n;
        let cy: f32 = members.iter().map(|&i| store.y[i]).sum::<f32>() / n;
        let cz: f32 = members.iter().map(|&i| store.z[i]).sum::<f32>() / n;
        org_centroids.insert(*oid, (cx, cy, cz));
    }

    // ── Innovation ────────────────────────────────────────────────────────
    for &(oid, ref members, fitness, pred_count, energy, colony_id, has_metacog, current_meme) in
        &org_snapshot
    {
        if current_meme != 0 {
            continue; // Already has a meme
        }

        if rng.next() >= 0.003 {
            continue; // Innovation probability
        }

        // Choose meme base by environmental context
        let base: u16 = if pred_count > 2 {
            9 // Defensive (9-12)
        } else if fitness > 10.0 {
            5 // Social (5-8)
        } else if energy > members.len() as f32 * 5.0 {
            13 // Tool/build (13-16)
        } else {
            1 // Foraging (1-4)
        };

        let meme = base + (rng.next() * 4.0).floor() as u16;

        // Set meme on all org particles
        for &idx in members {
            store.cultural_meme[idx] = meme;
        }

        // Add to org cultural memory (max 8 entries)
        if let Some(oinfo) = org_reg.organisms.get_mut(&oid) {
            if oinfo.cultural_memory.len() >= 8 {
                oinfo.cultural_memory.remove(0);
            }
            oinfo.cultural_memory.push(meme);
        }

        cultural_count.0 += 1;

        events.push(
            tick,
            format!("Org {} innovated meme {}", oid, meme),
            EventType::Culture,
        );
    }

    // ── Transmission ──────────────────────────────────────────────────────
    let trans_range = ir * 3.0;
    let trans_range_sq = trans_range * trans_range;

    // Collect transmissions to apply after iteration
    let mut transmissions: Vec<(u32, u16, f32, f32)> = Vec::new(); // (target_oid, meme, memory_share, epi_share)

    for &(oid, ref members, fitness, _pred, _energy, colony_id, has_metacog, meme) in &org_snapshot
    {
        if meme == 0 {
            continue; // No meme to transmit
        }

        let (cx, cy, cz) = match org_centroids.get(&oid) {
            Some(&c) => c,
            None => continue,
        };

        // Compute avg memory and epi_weight for behavioral learning transfer
        let avg_memory = if !members.is_empty() {
            members.iter().map(|&i| store.memory[i]).sum::<f32>() / members.len() as f32
        } else {
            0.0
        };
        let avg_epi = if !members.is_empty() {
            members.iter().map(|&i| store.epi_weight[i]).sum::<f32>() / members.len() as f32
        } else {
            1.0
        };

        // Find nearby organisms without this meme
        for &(target_oid, ref target_members, target_fitness, _, _, target_colony, target_metacog, target_meme) in
            &org_snapshot
        {
            if target_oid == oid {
                continue;
            }
            if target_meme != 0 {
                continue; // Already has a meme
            }

            let (tx, ty, tz) = match org_centroids.get(&target_oid) {
                Some(&c) => c,
                None => continue,
            };

            // Distance check (not wrapped for simplicity; could improve)
            let dx = cx - tx;
            let dy = cy - ty;
            let dz = cz - tz;
            let dsq = dx * dx + dy * dy + dz * dz;

            if dsq > trans_range_sq {
                continue;
            }

            // Base transmission probability
            let mut prob: f32 = 0.05;

            // Boost by fitness gap (higher fitness → more "prestigious" meme)
            if fitness > target_fitness {
                prob += (fitness - target_fitness) * 0.005;
            }

            // Boost if same colony
            if colony_id >= 0 && colony_id == target_colony {
                prob += 0.02;
            }

            // Boost if target has metacognition (better learner)
            if target_metacog {
                prob += 0.03;
            }

            // Boost if either has active symbols
            let has_symbols = members.iter().any(|&i| store.symbol_code[i] > 0);
            if has_symbols {
                prob += 0.01;
            }

            if rng.next() < prob {
                transmissions.push((target_oid, meme, avg_memory, avg_epi));
            }
        }
    }

    // Apply transmissions
    for (target_oid, meme, src_memory, src_epi) in transmissions {
        if let Some(oinfo) = org_reg.organisms.get(&target_oid) {
            let target_members: Vec<usize> = oinfo
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
                .collect();

            for &idx in &target_members {
                // Transfer meme
                store.cultural_meme[idx] = meme;

                // 20% behavioral learning: memory and epi_weight drift toward source
                store.memory[idx] += (src_memory - store.memory[idx]) * 0.2;
                store.epi_weight[idx] += (src_epi - store.epi_weight[idx]) * 0.2;
            }
        }

        // Add to target org cultural memory
        if let Some(oinfo) = org_reg.organisms.get_mut(&target_oid) {
            if oinfo.cultural_memory.len() >= 8 {
                oinfo.cultural_memory.remove(0);
            }
            oinfo.cultural_memory.push(meme);
        }

        cultural_count.0 += 1;

        events.push(
            tick,
            format!("Meme {} transmitted to org {}", meme, target_oid),
            EventType::Culture,
        );
    }

    // Apply behavioral effects after all transmissions
    apply_meme_effects(&mut store);
}

pub fn culture_system(
    mut store: ResMut<ParticleStore>,
    mut org_reg: ResMut<OrganismRegistry>,
    config: Res<SimConfig>,
    counters: Res<SimCounters>,
    mut events: ResMut<EventLog>,
    mut cultural_count: ResMut<CulturalEventCount>,
    mut fields: ResMut<SimFields>,
    mut rng: ResMut<SimRng>,
) {
    culture_inner(&mut *store, &mut *org_reg, &*config, &*counters, &mut *events, &mut *cultural_count, &mut *fields, &mut *rng);
}

/// Apply behavioral effects of cultural memes to particles.
///
/// Meme categories:
///   1-4   (foraging):   Catalyst signal+0.04, energy+0.005
///   5-8   (social):     Membrane signal+0.03, epi+0.0005, energy+0.003
///   9-12  (defensive):  Motor/Defense signal+0.06, defense energy+0.008
///   13-16 (tool/build): Motor signal+0.025, Data gene_expr+0.04
fn apply_meme_effects(store: &mut ParticleStore) {
    let len = store.id.len();

    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }

        let meme = store.cultural_meme[i];
        if meme == 0 {
            continue;
        }

        match meme {
            1..=4 => {
                // Foraging memes: boost Catalyst sensing & energy
                if store.ptype[i] == ParticleType::Catalyst {
                    store.signal[i] = (store.signal[i] + 0.04).clamp(-1.0, 1.0);
                }
                store.energy[i] += 0.005;
            }
            5..=8 => {
                // Social memes: boost Membrane signal, epi, energy
                if store.ptype[i] == ParticleType::Membrane {
                    store.signal[i] = (store.signal[i] + 0.03).clamp(-1.0, 1.0);
                }
                store.epi_weight[i] += 0.0005;
                store.energy[i] += 0.003;
            }
            9..=12 => {
                // Defensive memes: boost Motor/Defense particles
                if store.ptype[i] == ParticleType::Motor
                    || store.cell_role[i] == CellRole::Defense
                {
                    store.signal[i] = (store.signal[i] + 0.06).clamp(-1.0, 1.0);
                }
                if store.cell_role[i] == CellRole::Defense {
                    store.energy[i] += 0.008;
                }
            }
            13..=16 => {
                // Tool/build memes: boost Motor signal & Data gene_expr
                if store.ptype[i] == ParticleType::Motor {
                    store.signal[i] = (store.signal[i] + 0.025).clamp(-1.0, 1.0);
                }
                if store.ptype[i] == ParticleType::Data {
                    store.gene_expr[i] = (store.gene_expr[i] + 0.04).clamp(0.0, 1.0);
                }
            }
            _ => {
                // Unknown meme — no effect
            }
        }
    }
}

// ─── P4.4: Meta-Cognition System ───────────────────────────────────────────
//
// For organisms with >6 particles and ≥3 Data particles:
//   1. Build a Data→Data adjacency graph (bonded Data within same org)
//   2. BFS to find connected chains of Data particles
//   3. Chain length ≥3 → meta-cognition depth = floor(chain_length / 3)
//
// Effects by layer:
//   Layer 0 (base): enhanced gene regulation (gene_expr * 1.1 + 0.05)
//   Layer 1+ (meta): self-modulation — higher Data modulates lower Data
//                     based on environmental signals
//
// Global bonuses for meta-cognitive organisms:
//   - Reduce extreme epi_weight (multiply by 0.98)
//   - Enhanced learning (memory converges if energy > 5)
//   - Signal coherence (0.97 * signal + 0.03 * memory)

pub fn meta_cognition_inner(
    store: &mut ParticleStore,
    org_reg: &mut OrganismRegistry,
    events: &mut EventLog,
    counters: &SimCounters,
    metacog_count: &mut MetaCogOrgCount,
) {
    let tick = counters.tick;
    metacog_count.0 = 0;

    let org_ids: Vec<u32> = org_reg.organisms.keys().copied().collect();

    for oid in org_ids {
        let oinfo = match org_reg.organisms.get(&oid) {
            Some(o) => o,
            None => continue,
        };

        // Collect alive, non-deposit members
        let member_indices: Vec<usize> = oinfo
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
            .collect();

        let size = member_indices.len();
        if size <= 6 {
            continue;
        }

        // Collect Data particle indices within this organism
        let data_indices: Vec<usize> = member_indices
            .iter()
            .copied()
            .filter(|&idx| store.ptype[idx] == ParticleType::Data)
            .collect();

        if data_indices.len() < 3 {
            continue;
        }

        // ── Build Data→Data adjacency graph ──────────────────────────────
        // Map store index → local index within data_indices
        let mut idx_to_local: HashMap<usize, usize> = HashMap::new();
        for (local, &idx) in data_indices.iter().enumerate() {
            idx_to_local.insert(idx, local);
        }

        let n = data_indices.len();
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (local, &idx) in data_indices.iter().enumerate() {
            for &bid in &store.bonds[idx] {
                if let Some(&bi) = store.id_to_index.get(&bid) {
                    if let Some(&other_local) = idx_to_local.get(&bi) {
                        adjacency[local].push(other_local);
                    }
                }
            }
        }

        // ── BFS to find longest connected chain ─────────────────────────
        let mut visited = vec![false; n];
        let mut max_chain_len: usize = 0;
        let mut longest_chain: Vec<usize> = Vec::new();

        for start in 0..n {
            if visited[start] {
                continue;
            }

            // BFS from this start node
            let mut queue: VecDeque<usize> = VecDeque::new();
            let mut component: Vec<usize> = Vec::new();

            queue.push_back(start);
            visited[start] = true;

            while let Some(node) = queue.pop_front() {
                component.push(node);
                for &neighbor in &adjacency[node] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }

            if component.len() > max_chain_len {
                max_chain_len = component.len();
                longest_chain = component;
            }
        }

        // Meta-cognition requires chain length ≥ 3
        if max_chain_len < 3 {
            continue;
        }

        let meta_depth = (max_chain_len / 3) as u32;
        let meta_depth_f32 = meta_depth as f32;

        metacog_count.0 += 1;

        // Update organism info
        if let Some(oinfo) = org_reg.organisms.get_mut(&oid) {
            let prev_depth = oinfo.meta_cog_depth;
            oinfo.meta_cog_depth = meta_depth_f32;

            // Log first achievement of meta-cognition
            if prev_depth == 0.0 && meta_depth >= 1 {
                events.push(
                    tick,
                    format!(
                        "Org {} achieved meta-cognition depth {} (chain={})",
                        oid, meta_depth, max_chain_len
                    ),
                    EventType::Metacog,
                );
            }
        }

        // Set meta_cog_level on all org particles
        for &idx in &member_indices {
            store.meta_cog_level[idx] = meta_depth_f32;
        }

        // ── Layer 0: Enhanced gene regulation ────────────────────────────
        // All Data particles in the chain get enhanced gene expression
        for &local in &longest_chain {
            let idx = data_indices[local];
            store.gene_expr[idx] = (store.gene_expr[idx] * 1.1 + 0.05).clamp(0.0, 1.0);
        }

        // ── Layer 1+: Self-modulation ────────────────────────────────────
        // Higher Data in the chain modulates lower Data based on env signal
        if meta_depth >= 1 && longest_chain.len() >= 2 {
            // Compute organism energy average as environmental signal
            let org_energy = member_indices
                .iter()
                .map(|&i| store.energy[i])
                .sum::<f32>();
            let env_signal = (org_energy / (size as f32 * 5.0)).clamp(0.0, 1.0);

            // Process chain in layers: later elements modulate earlier ones
            for layer in 0..(longest_chain.len() - 1) {
                let higher_local = longest_chain[layer + 1];
                let lower_local = longest_chain[layer];

                let higher_idx = data_indices[higher_local];
                let lower_idx = data_indices[lower_local];

                // Modulation: higher Data influences lower Data
                let higher_memory = store.memory[higher_idx];
                let modulation = env_signal * higher_memory * 0.05;

                // Apply modulation to lower Data
                store.gene_expr[lower_idx] =
                    (store.gene_expr[lower_idx] + modulation).clamp(0.0, 1.0);

                // Signal drifts toward environmental signal
                store.signal[lower_idx] +=
                    (env_signal - store.signal[lower_idx]) * 0.05;

                // Memory drifts toward normalized org energy
                store.memory[lower_idx] +=
                    (env_signal - store.memory[lower_idx]) * 0.03;
            }
        }

        // ── Global bonuses for meta-cognitive organisms ──────────────────
        for &idx in &member_indices {
            // Reduce extreme epi_weight (homeostatic regulation)
            if store.epi_weight[idx].abs() > 0.5 {
                store.epi_weight[idx] *= 0.98;
            }

            // Enhanced learning: memory converges if energy > 5
            if store.energy[idx] > 5.0 {
                let target_memory = store.energy[idx] / (size as f32 * 5.0);
                store.memory[idx] +=
                    (target_memory - store.memory[idx]) * 0.01;
            }

            // Signal coherence: blend signal with memory
            store.signal[idx] =
                0.97 * store.signal[idx] + 0.03 * store.memory[idx];
        }
    }
}

pub fn meta_cognition_system(
    mut store: ResMut<ParticleStore>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut events: ResMut<EventLog>,
    counters: Res<SimCounters>,
    mut metacog_count: ResMut<MetaCogOrgCount>,
) {
    meta_cognition_inner(&mut *store, &mut *org_reg, &mut *events, &*counters, &mut *metacog_count);
}
