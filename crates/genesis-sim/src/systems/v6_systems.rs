// ── v6_systems.rs ── Immune, Symbiogenesis, Sexual Reproduction, Niches ─────
//
// This module ports four systems from the TypeScript v6 implementation:
//   V6.2 — Immune Signatures:  type-composition-based organism signatures
//   V6.3 — Symbiogenesis:      organism merger after sustained contact
//   V6.5 — Sexual Reproduction: two-parent reproduction with crossover
//   M3   — Ecological Niches:  type-specific environmental bonuses

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::{ParticleStore, SimRng};

use std::collections::{HashMap, HashSet};

// ─── V6.2: Immune Signatures ───────────────────────────────────────────────
//
// Every 10 ticks, compute a hash-like signature for each organism based on
// its type composition.  The signature is set on all member particles and
// stored in OrgSignatures for cross-referencing (e.g. predation immunity).
//
// Algorithm:
//   sig = seed & 0xFFFF
//   for each type: sig = ((sig * 7 + floor(count / 2)) & 0xFFFF)

pub fn immune_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
    org_reg: &mut OrganismRegistry,
    org_sigs: &mut OrgSignatures,
    counters: &SimCounters,
) {
    // Only recompute every 10 ticks
    if counters.tick % 10 != 0 {
        return;
    }

    let seed = config.seed;
    org_sigs.0.clear();

    let org_ids: Vec<u32> = org_reg.organisms.keys().copied().collect();

    for oid in org_ids {
        let oinfo = match org_reg.organisms.get(&oid) {
            Some(o) => o,
            None => continue,
        };

        // Count types in the organism
        let mut type_counts = [0u32; NUM_TYPES];
        let mut member_indices: Vec<usize> = Vec::new();

        for &pid in &oinfo.members {
            if let Some(&idx) = store.id_to_index.get(&pid) {
                if store.alive[idx] && !store.is_deposit[idx] {
                    type_counts[store.ptype[idx].as_index()] += 1;
                    member_indices.push(idx);
                }
            }
        }

        if member_indices.is_empty() {
            continue;
        }

        // Compute immune signature from type composition
        let mut sig: u32 = seed & 0xFFFF;
        for t in 0..NUM_TYPES {
            sig = (sig.wrapping_mul(7).wrapping_add(type_counts[t] / 2)) & 0xFFFF;
        }

        // Store the signature
        org_sigs.0.insert(oid, sig);

        // Set signature on all member particles
        for &idx in &member_indices {
            store.signature[idx] = sig;
        }
    }
}

pub fn immune_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut org_sigs: ResMut<OrgSignatures>,
    counters: Res<SimCounters>,
) {
    immune_inner(&mut *store, &*config, &mut *org_reg, &mut *org_sigs, &*counters);
}

// ─── V6.3: Symbiogenesis ───────────────────────────────────────────────────
//
// When two organisms with Membrane particles remain in close contact
// (within bond_distance * 2) for 200+ ticks, the larger absorbs the smaller.
//
// Process:
//   1. Track contact duration in ContactTracker
//   2. After 200 ticks and combined size ≤ 50:
//      a. Determine keeper (larger) and donor (smaller)
//      b. Form up to 3 bonds between membrane particles
//      c. Transfer all donor particles to keeper organism
//      d. Merge stats (energy, fitness, etc.)
//      e. Remove donor organism

pub fn symbiogenesis_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
    org_reg: &mut OrganismRegistry,
    contacts: &mut ContactTracker,
    counters: &mut SimCounters,
    events: &mut EventLog,
    phylogeny: &mut PhylogenyTree,
) {
    let ws = config.world_size;
    let contact_range = config.bond_distance * 2.0;
    let contact_range_sq = contact_range * contact_range;
    let tick = counters.tick;

    // Collect organism data for proximity checks
    struct OrgData {
        oid: u32,
        centroid: (f32, f32, f32),
        size: usize,
        has_membrane: bool,
        membrane_indices: Vec<usize>,
        all_member_pids: Vec<u32>,
    }

    let org_data: Vec<OrgData> = org_reg
        .organisms
        .iter()
        .filter_map(|(&oid, oinfo)| {
            let mut cx = 0.0f32;
            let mut cy = 0.0f32;
            let mut cz = 0.0f32;
            let mut count = 0usize;
            let mut has_membrane = false;
            let mut membrane_indices = Vec::new();
            let mut all_member_pids = Vec::new();

            for &pid in &oinfo.members {
                if let Some(&idx) = store.id_to_index.get(&pid) {
                    if store.alive[idx] && !store.is_deposit[idx] {
                        cx += store.x[idx];
                        cy += store.y[idx];
                        cz += store.z[idx];
                        count += 1;
                        all_member_pids.push(pid);
                        if store.ptype[idx] == ParticleType::Membrane {
                            has_membrane = true;
                            membrane_indices.push(idx);
                        }
                    }
                }
            }

            if count == 0 {
                return None;
            }

            Some(OrgData {
                oid,
                centroid: (cx / count as f32, cy / count as f32, cz / count as f32),
                size: count,
                has_membrane,
                membrane_indices,
                all_member_pids,
            })
        })
        .collect();

    // Check all organism pairs for contact
    let mut new_contacts: HashMap<(u32, u32), bool> = HashMap::new();
    let mut merges_to_do: Vec<(u32, u32)> = Vec::new(); // (keeper, donor)

    for i in 0..org_data.len() {
        if !org_data[i].has_membrane {
            continue;
        }
        for j in (i + 1)..org_data.len() {
            if !org_data[j].has_membrane {
                continue;
            }

            let (ax, ay, az) = org_data[i].centroid;
            let (bx, by, bz) = org_data[j].centroid;

            // Simple distance check (not wrapped for centroids — close enough)
            let dx = ax - bx;
            let dy = ay - by;
            let dz = az - bz;
            let dsq = dx * dx + dy * dy + dz * dz;

            if dsq > contact_range_sq * 4.0 {
                continue; // Too far apart (using wider check for organism centroids)
            }

            // Check if any membrane particles are actually within range
            let mut in_contact = false;
            'outer: for &mi in &org_data[i].membrane_indices {
                for &mj in &org_data[j].membrane_indices {
                    let d = store.distance_sq_wrapped(mi, mj, ws);
                    if d < contact_range_sq {
                        in_contact = true;
                        break 'outer;
                    }
                }
            }

            if !in_contact {
                continue;
            }

            // Canonicalize pair key (smaller id first)
            let a_id = org_data[i].oid;
            let b_id = org_data[j].oid;
            let key = if a_id < b_id {
                (a_id, b_id)
            } else {
                (b_id, a_id)
            };

            new_contacts.insert(key, true);

            // Increment contact counter
            let contact_ticks = contacts.0.entry(key).or_insert(0);
            *contact_ticks += 10; // We run every tick but check every 10

            // Check if ready for symbiogenesis
            if *contact_ticks >= 200 {
                let combined_size = org_data[i].size + org_data[j].size;
                if combined_size <= 50 {
                    // Determine keeper (larger) and donor (smaller)
                    let (keeper, donor) = if org_data[i].size >= org_data[j].size {
                        (a_id, b_id)
                    } else {
                        (b_id, a_id)
                    };
                    merges_to_do.push((keeper, donor));
                }
            }
        }
    }

    // Remove stale contacts (pairs no longer in proximity)
    let stale_keys: Vec<(u32, u32)> = contacts
        .0
        .keys()
        .filter(|k| !new_contacts.contains_key(k))
        .copied()
        .collect();
    for key in stale_keys {
        contacts.0.remove(&key);
    }

    // ── Execute merges ──────────────────────────────────────────────────
    for (keeper_id, donor_id) in merges_to_do {
        // Get donor data
        let donor_info = match org_reg.organisms.remove(&donor_id) {
            Some(d) => d,
            None => continue,
        };

        let donor_pids: Vec<u32> = donor_info.members.iter().copied().collect();

        // Get membrane indices for bond formation
        let keeper_membranes: Vec<usize> = {
            match org_reg.organisms.get(&keeper_id) {
                Some(o) => o
                    .members
                    .iter()
                    .filter_map(|&pid| {
                        let idx = *store.id_to_index.get(&pid)?;
                        if store.alive[idx]
                            && !store.is_deposit[idx]
                            && store.ptype[idx] == ParticleType::Membrane
                        {
                            Some(idx)
                        } else {
                            None
                        }
                    })
                    .collect(),
                None => continue,
            }
        };

        let donor_membranes: Vec<usize> = donor_pids
            .iter()
            .filter_map(|&pid| {
                let idx = *store.id_to_index.get(&pid)?;
                if store.alive[idx]
                    && !store.is_deposit[idx]
                    && store.ptype[idx] == ParticleType::Membrane
                {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        // Form up to 3 bonds between membrane particles
        let mut bonds_formed = 0;
        for &ki in &keeper_membranes {
            if bonds_formed >= 3 {
                break;
            }
            for &di in &donor_membranes {
                if bonds_formed >= 3 {
                    break;
                }
                if store.can_bond(ki) && store.can_bond(di) {
                    store.form_bond(ki, di);
                    bonds_formed += 1;
                }
            }
        }

        // Transfer all donor particles to keeper organism
        for &pid in &donor_pids {
            if let Some(&idx) = store.id_to_index.get(&pid) {
                store.organism_id[idx] = keeper_id as i32;
            }
        }

        // Merge stats into keeper
        if let Some(keeper) = org_reg.organisms.get_mut(&keeper_id) {
            for pid in donor_pids {
                keeper.members.insert(pid);
            }
            keeper.energy += donor_info.energy;
            keeper.fitness += donor_info.fitness * 0.5;
            keeper.predation_count += donor_info.predation_count;
            keeper.deposit_count += donor_info.deposit_count;
            keeper.tool_use_count += donor_info.tool_use_count;
            keeper.build_score += donor_info.build_score;

            // Merge cultural memory (deduplicate, cap at 8)
            for meme in &donor_info.cultural_memory {
                if !keeper.cultural_memory.contains(meme) {
                    if keeper.cultural_memory.len() >= 8 {
                        keeper.cultural_memory.remove(0);
                    }
                    keeper.cultural_memory.push(*meme);
                }
            }
        }

        // Clean up contact tracker entries involving donor
        let remove_keys: Vec<(u32, u32)> = contacts
            .0
            .keys()
            .filter(|(a, b)| *a == donor_id || *b == donor_id)
            .copied()
            .collect();
        for key in remove_keys {
            contacts.0.remove(&key);
        }

        counters.total_symbiogenesis += 1;

        events.push(
            tick,
            format!(
                "Symbiogenesis: org {} absorbed org {} ({} bonds formed)",
                keeper_id, donor_id, bonds_formed
            ),
            EventType::Symbiogenesis,
        );

        // Record in phylogeny
        phylogeny.add(
            keeper_id,
            donor_id as i32,
            tick,
            org_reg
                .organisms
                .get(&keeper_id)
                .map(|o| o.generation)
                .unwrap_or(0),
            org_reg
                .organisms
                .get(&keeper_id)
                .map(|o| o.members.len())
                .unwrap_or(0),
        );
    }
}

pub fn symbiogenesis_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut contacts: ResMut<ContactTracker>,
    mut counters: ResMut<SimCounters>,
    mut events: ResMut<EventLog>,
    mut phylogeny: ResMut<PhylogenyTree>,
) {
    symbiogenesis_inner(&mut *store, &*config, &mut *org_reg, &mut *contacts, &mut *counters, &mut *events, &mut *phylogeny);
}

// ─── V6.5: Sexual Reproduction ─────────────────────────────────────────────
//
// Two "ready" organisms can combine genetic material to produce offspring.
//
// Readiness conditions:
//   • repro_cooldown == 0
//   • size ≥ 4
//   • energy > size * 1.5 + 2
//   • fitness > 5, or (generation ≥ 1 and rng < 0.1)
//
// Partners must be within interaction_radius * 3 and size ratio < 2.5.
//
// Offspring:
//   • Size = min(floor((sA + sB) / 4), 8)
//   • Types alternate from parents with mutation
//   • Chain bonds between child particles, close loop if 4+

pub fn sexual_reproduce_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
    org_reg: &mut OrganismRegistry,
    counters: &mut SimCounters,
    events: &mut EventLog,
    fields: &mut SimFields,
    phylogeny: &mut PhylogenyTree,
    rng: &mut SimRng,
) {
    let ws = config.world_size;
    let ir = config.interaction_radius;
    let pair_range = ir * 3.0;
    let pair_range_sq = pair_range * pair_range;
    let mutation_rate = config.mutation_rate;
    let tick = counters.tick;

    let max_p = config.max_particles;
    let total_alive = store.alive.iter().filter(|&&a| a).count();
    if total_alive + 20 >= max_p {
        return; // Global cap
    }

    // Collect ready organisms
    struct ReadyOrg {
        oid: u32,
        centroid: (f32, f32, f32),
        size: usize,
        energy: f32,
        fitness: f32,
        generation: u32,
        types: Vec<ParticleType>,
        member_indices: Vec<usize>,
    }

    let ready_orgs: Vec<ReadyOrg> = org_reg
        .organisms
        .iter()
        .filter_map(|(&oid, oinfo)| {
            if oinfo.repro_cooldown > 0 {
                return None;
            }

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
            if size < 4 {
                return None;
            }

            let energy = oinfo.energy;
            if energy <= size as f32 * 1.5 + 2.0 {
                return None;
            }

            // Fitness check (relaxed for established lineages)
            let fitness_ok =
                oinfo.fitness > 5.0 || (oinfo.generation >= 1);

            if !fitness_ok {
                return None;
            }

            let (mut cx, mut cy, mut cz) = (0.0f32, 0.0f32, 0.0f32);
            let mut types = Vec::new();
            for &idx in &member_indices {
                cx += store.x[idx];
                cy += store.y[idx];
                cz += store.z[idx];
                types.push(store.ptype[idx]);
            }
            let n = size as f32;

            Some(ReadyOrg {
                oid,
                centroid: (cx / n, cy / n, cz / n),
                size,
                energy,
                fitness: oinfo.fitness,
                generation: oinfo.generation,
                types,
                member_indices,
            })
        })
        .collect();

    // Find compatible pairs
    let mut used: HashSet<u32> = HashSet::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();

    for i in 0..ready_orgs.len() {
        if used.contains(&ready_orgs[i].oid) {
            continue;
        }
        for j in (i + 1)..ready_orgs.len() {
            if used.contains(&ready_orgs[j].oid) {
                continue;
            }

            let (ax, ay, az) = ready_orgs[i].centroid;
            let (bx, by, bz) = ready_orgs[j].centroid;

            let dx = ax - bx;
            let dy = ay - by;
            let dz = az - bz;
            let dsq = dx * dx + dy * dy + dz * dz;

            if dsq > pair_range_sq {
                continue;
            }

            // Size ratio check
            let ratio = if ready_orgs[i].size >= ready_orgs[j].size {
                ready_orgs[i].size as f32 / ready_orgs[j].size as f32
            } else {
                ready_orgs[j].size as f32 / ready_orgs[i].size as f32
            };

            if ratio > 2.5 {
                continue;
            }

            // For gen >= 1 parents, apply rng check
            let a = &ready_orgs[i];
            let b = &ready_orgs[j];
            if a.fitness <= 5.0 && a.generation >= 1 && rng.next() >= 0.1 {
                continue;
            }
            if b.fitness <= 5.0 && b.generation >= 1 && rng.next() >= 0.1 {
                continue;
            }

            pairs.push((i, j));
            used.insert(ready_orgs[i].oid);
            used.insert(ready_orgs[j].oid);
            break; // Each org can only mate once per tick
        }
    }

    // ── Spawn children ──────────────────────────────────────────────────
    for (ai, bi) in pairs {
        let a = &ready_orgs[ai];
        let b = &ready_orgs[bi];

        let child_size = ((a.size + b.size) / 4).min(8).max(2);

        // Midpoint between parents as spawn location
        let spawn_x = (a.centroid.0 + b.centroid.0) * 0.5;
        let spawn_y = (a.centroid.1 + b.centroid.1) * 0.5;
        let spawn_z = (a.centroid.2 + b.centroid.2) * 0.5;

        // Create child organism
        let child_oid = org_reg.create();
        let child_gen = a.generation.max(b.generation) + 1;

        if let Some(child_info) = org_reg.organisms.get_mut(&child_oid) {
            child_info.generation = child_gen;
            child_info.parent_id = a.oid as i32;
            child_info.repro_cooldown = 80;
        }

        // Alternate types from parents, with mutation
        let mut child_indices: Vec<usize> = Vec::new();
        let energy_per = (a.energy + b.energy) / (4.0 * child_size as f32);

        for ci in 0..child_size {
            // Alternate type selection from parent A and B
            let base_type = if ci % 2 == 0 {
                a.types[ci % a.types.len()]
            } else {
                b.types[ci % b.types.len()]
            };

            // Mutation: chance to change type
            let ptype = if rng.next() < mutation_rate {
                ParticleType::from_index(rng.next() as usize % NUM_TYPES)
            } else {
                base_type
            };

            // Spawn position: offset from midpoint in a small cluster
            let angle = ci as f32 * std::f32::consts::TAU / child_size as f32;
            let offset = 0.5;
            let px = spawn_x + angle.cos() * offset;
            let py = spawn_y + angle.sin() * offset;
            let pz = spawn_z + (rng.next() - 0.5) * 0.3;

            let idx = store.spawn(ptype, px, py, pz, energy_per, &mut rng);
            store.organism_id[idx] = child_oid as i32;

            // Inherit some epigenetic weight from parents
            let parent_epi = if ci % 2 == 0 {
                if !a.member_indices.is_empty() {
                    store.epi_weight[a.member_indices[ci % a.member_indices.len()]]
                } else {
                    1.0
                }
            } else {
                if !b.member_indices.is_empty() {
                    store.epi_weight[b.member_indices[ci % b.member_indices.len()]]
                } else {
                    1.0
                }
            };
            store.epi_weight[idx] = parent_epi;

            // Register as member
            let pid = store.id[idx];
            if let Some(child_info) = org_reg.organisms.get_mut(&child_oid) {
                child_info.members.insert(pid);
            }

            child_indices.push(idx);
        }

        // Chain bonds between sequential child particles
        for k in 0..(child_indices.len() - 1) {
            store.form_bond(child_indices[k], child_indices[k + 1]);
        }

        // Close loop if 4+ particles
        if child_indices.len() >= 4 {
            let first = child_indices[0];
            let last = *child_indices.last().unwrap();
            store.form_bond(first, last);
        }

        // Deduct energy from parents
        let cost_per_parent = energy_per * child_size as f32 * 0.5;
        for &idx in &a.member_indices {
            store.energy[idx] -= cost_per_parent / a.size as f32;
        }
        for &idx in &b.member_indices {
            store.energy[idx] -= cost_per_parent / b.size as f32;
        }

        // Set cooldowns on parents
        if let Some(pa) = org_reg.organisms.get_mut(&a.oid) {
            pa.repro_cooldown = 200;
        }
        if let Some(pb) = org_reg.organisms.get_mut(&b.oid) {
            pb.repro_cooldown = 200;
        }

        // Inject pheromone at spawn location
        fields.phero_attr.inject(spawn_x, spawn_y, spawn_z, ws, 0.3);

        // Record phylogeny
        phylogeny.add(child_oid, a.oid as i32, tick, child_gen, child_size);

        counters.total_sexual_repro += 1;

        events.push(
            tick,
            format!(
                "Sexual reproduction: org {} × org {} → child org {} (size={}, gen={})",
                a.oid, b.oid, child_oid, child_size, child_gen
            ),
            EventType::Sexual,
        );
    }
}

pub fn sexual_reproduce_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut counters: ResMut<SimCounters>,
    mut events: ResMut<EventLog>,
    mut fields: ResMut<SimFields>,
    mut phylogeny: ResMut<PhylogenyTree>,
    mut rng: ResMut<SimRng>,
) {
    sexual_reproduce_inner(&mut *store, &*config, &mut *org_reg, &mut *counters, &mut *events, &mut *fields, &mut *phylogeny, &mut *rng);
}

// ─── M3: Ecological Niches ─────────────────────────────────────────────────
//
// For organisms with 3+ particles, apply a per-tick energy bonus based on
// the dominant particle type and environmental conditions.
//
// Niche bonuses:
//   Catalyst:  near vent → 0.03 * ratio
//   Beta:      0.02 * ratio * solar * 5
//   Motor:     0.015 * ratio
//   Membrane:  0.02 * ratio
//   Data:      0.01 * ratio
//   Alpha:     0.015 * ratio

pub fn niche_bonuses_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
    org_reg: &OrganismRegistry,
    vents: &VentList,
) {
    let ws = config.world_size;
    let solar = config.solar_strength;

    for (&oid, oinfo) in &org_reg.organisms {
        // Collect alive, non-deposit member indices
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
        if size < 3 {
            continue;
        }

        // Count types to find dominant
        let mut type_counts = [0u32; NUM_TYPES];
        for &idx in &member_indices {
            type_counts[store.ptype[idx].as_index()] += 1;
        }

        let (dominant_type, dominant_count) = type_counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(t, &c)| (t, c))
            .unwrap_or((0, 0));

        let ratio = dominant_count as f32 / size as f32;

        // Compute organism centroid for vent proximity check
        let (mut cx, mut cy, mut cz) = (0.0f32, 0.0f32, 0.0f32);
        for &idx in &member_indices {
            cx += store.x[idx];
            cy += store.y[idx];
            cz += store.z[idx];
        }
        let n = size as f32;
        cx /= n;
        cy /= n;
        cz /= n;

        // Check if near any vent
        let near_vent = vents.0.iter().any(|v| {
            let dx = cx - v.position.x;
            let dy = cy - v.position.y;
            let dz = cz - v.position.z;
            let dsq = dx * dx + dy * dy + dz * dz;
            dsq < v.radius * v.radius * 4.0 // Within 2× vent radius
        });

        // Compute niche bonus based on dominant type
        let bonus = match ParticleType::from_index(dominant_type) {
            ParticleType::Catalyst => {
                if near_vent {
                    0.03 * ratio
                } else {
                    0.005 * ratio // Reduced bonus away from vents
                }
            }
            ParticleType::Beta => 0.02 * ratio * solar * 5.0,
            ParticleType::Motor => 0.015 * ratio,
            ParticleType::Membrane => 0.02 * ratio,
            ParticleType::Data => 0.01 * ratio,
            ParticleType::Alpha => 0.015 * ratio,
        };

        // Apply bonus to all member particles
        for &idx in &member_indices {
            store.energy[idx] += bonus;
        }
    }
}

pub fn niche_bonuses_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    org_reg: Res<OrganismRegistry>,
    vents: Res<VentList>,
) {
    niche_bonuses_inner(&mut *store, &*config, &*org_reg, &*vents);
}
