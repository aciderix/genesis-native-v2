// ── reproduction.rs ── Organism binary fission ──────────────────────────────
//
// Port of `reproduce()` from TypeScript.
// When an organism meets the size, energy, and cooldown thresholds it splits
// via binary fission into two child organisms.

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::{ParticleStore, SimRng};

use std::f32::consts::TAU;

/// Reproduce eligible organisms via binary fission.
///
/// Conditions for reproduction:
///   • Organism has ≥ 6 alive (non-deposit) particles
///   • Energy > particle_count * 3.0 + 8
///   • repro_cooldown == 0
///   • Total alive particles < max_particles − 20
///
/// Process:
///   1. Compute organism centroid.
///   2. Sort particles by distance from centroid.
///   3. Split into near half (stays in parent) and far half (new organism).
///   4. Each child gets half parent energy.
///   5. New organism members get slight random velocity perturbation.
///   6. Apply mutations to child particles at config.mutation_rate.
///   7. Set cooldowns: parent = 150, child = 100.
///   8. Child generation = parent.generation + 1.
///   9. Record phylogeny, inject wave & pheromone, emit event.
pub fn reproduce_inner(
    store: &mut ParticleStore,
    config: &SimConfig,
    org_reg: &mut OrganismRegistry,
    events: &mut EventLog,
    counters: &mut SimCounters,
    fields: &mut SimFields,
    phylogeny: &mut PhylogenyTree,
    rng: &mut SimRng,
) {
    let ws = config.world_size;
    let max_p = config.max_particles;
    let mutation_rate = config.mutation_rate;

    // Snapshot current alive count (approximate — we'll re-check each time)
    let total_alive = store.alive.iter().filter(|&&a| a).count();
    if total_alive + 20 >= max_p as usize {
        return; // global cap
    }

    // Collect organism ids to avoid borrow conflicts during iteration
    let org_ids: Vec<u32> = org_reg.organisms.keys().copied().collect();

    for oid in org_ids {
        // Re-fetch each time because we mutate the registry
        let oinfo = match org_reg.organisms.get(&oid) {
            Some(o) => o,
            None => continue,
        };

        // ── Eligibility checks ──────────────────────────────────────────
        if oinfo.repro_cooldown > 0 {
            continue;
        }

        // Collect alive, non-deposit member indices
        let alive_members: Vec<u32> = oinfo
            .members
            .iter()
            .copied()
            .filter(|&pid| {
                if let Some(&idx) = store.id_to_index.get(&pid) {
                    store.alive[idx] && !store.is_deposit[idx]
                } else {
                    false
                }
            })
            .collect();

        let member_count = alive_members.len();
        if member_count < 6 {
            continue;
        }
        if oinfo.energy < member_count as f32 * 3.0 + 8.0 {
            continue;
        }

        // Recheck global cap
        let current_alive = store.alive.iter().filter(|&&a| a).count();
        if current_alive + 20 >= max_p as usize {
            break;
        }

        // ── Compute centroid ────────────────────────────────────────────
        let (mut cx, mut cy, mut cz) = (0.0f32, 0.0f32, 0.0f32);
        for &pid in &alive_members {
            let idx = store.id_to_index[&pid];
            cx += store.x[idx];
            cy += store.y[idx];
            cz += store.z[idx];
        }
        let inv = 1.0 / member_count as f32;
        cx *= inv;
        cy *= inv;
        cz *= inv;

        // ── Sort by distance from centroid ──────────────────────────────
        let mut sorted: Vec<(u32, f32)> = alive_members
            .iter()
            .map(|&pid| {
                let idx = store.id_to_index[&pid];
                let dx = store.x[idx] - cx;
                let dy = store.y[idx] - cy;
                let dz = store.z[idx] - cz;
                (pid, dx * dx + dy * dy + dz * dz)
            })
            .collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Split: near half stays with parent, far half becomes new organism
        let split = member_count / 2;
        let parent_pids: Vec<u32> = sorted[..split].iter().map(|(pid, _)| *pid).collect();
        let child_pids: Vec<u32> = sorted[split..].iter().map(|(pid, _)| *pid).collect();

        if child_pids.is_empty() {
            continue;
        }

        // ── Energy partition ────────────────────────────────────────────
        let half_energy = oinfo.energy * 0.5;
        let parent_gen = oinfo.generation;
        let parent_id_for_phylo = oid;

        // ── Create new organism ─────────────────────────────────────────
        let new_oid = org_reg.next_id;
        org_reg.next_id += 1;

        // Build child OrganismInfo
        let mut child_members = std::collections::HashSet::new();
        for &pid in &child_pids {
            child_members.insert(pid);
        }

        let child_info = OrganismInfo {
            members: child_members,
            energy: half_energy,
            age: 0,
            generation: parent_gen + 1,
            fitness: 0.0,
            colony_id: -1,
            repro_cooldown: 100,
            predation_count: 0,
            deposit_count: 0,
            cells: std::collections::HashMap::new(),
            specialization: 0.0,
            symbol_history: Vec::new(),
            is_multicellular: child_pids.len() >= 4,
            tool_use_count: 0,
            build_score: 0.0,
            cultural_memory: Vec::new(),
            meta_cog_depth: 0.0,
            parent_id: parent_id_for_phylo as i32,
        };
        org_reg.organisms.insert(new_oid, child_info);

        // Update parent organism: remove child pids, set cooldown, halve energy
        if let Some(parent) = org_reg.organisms.get_mut(&oid) {
            for &pid in &child_pids {
                parent.members.remove(&pid);
            }
            parent.energy = half_energy;
            parent.repro_cooldown = 150;
        }

        // ── Update particle organism_id for child members ───────────────
        // Also apply random velocity perturbation to child particles.
        let child_centroid = {
            let (mut ccx, mut ccy, mut ccz) = (0.0f32, 0.0f32, 0.0f32);
            for &pid in &child_pids {
                let idx = store.id_to_index[&pid];
                ccx += store.x[idx];
                ccy += store.y[idx];
                ccz += store.z[idx];
            }
            let cinv = 1.0 / child_pids.len() as f32;
            (ccx * cinv, ccy * cinv, ccz * cinv)
        };

        for &pid in &child_pids {
            if let Some(&idx) = store.id_to_index.get(&pid) {
                store.organism_id[idx] = new_oid as i32;

                // Slight velocity perturbation away from parent centroid
                let dx = store.x[idx] - cx;
                let dy = store.y[idx] - cy;
                let dz = store.z[idx] - cz;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);
                let push = 0.05;
                store.vx[idx] += dx / dist * push + (rng.next() - 0.5) * 0.02;
                store.vy[idx] += dy / dist * push + (rng.next() - 0.5) * 0.02;
                store.vz[idx] += dz / dist * push + (rng.next() - 0.5) * 0.02;
            }
        }

        // ── Sever cross-organism bonds ──────────────────────────────────
        // Remove bonds between parent and child particles
        let parent_set: std::collections::HashSet<u32> = parent_pids.iter().copied().collect();
        let child_set: std::collections::HashSet<u32> = child_pids.iter().copied().collect();

        for &pid in &child_pids {
            if let Some(&idx) = store.id_to_index.get(&pid) {
                let to_remove: Vec<u32> = store.bonds[idx]
                    .iter()
                    .copied()
                    .filter(|b| parent_set.contains(b))
                    .collect();
                for bid in &to_remove {
                    store.bonds[idx].remove(bid);
                    if let Some(&j) = store.id_to_index.get(bid) {
                        store.bonds[j].remove(&pid);
                    }
                }
            }
        }

        // ── Mutations ───────────────────────────────────────────────────
        // For each child particle, chance to mutate type and genome
        for &pid in &child_pids {
            if let Some(&idx) = store.id_to_index.get(&pid) {
                if rng.next() < mutation_rate {
                    // Change type randomly (0..NUM_TYPES)
                    let new_type = (rng.next() * NUM_TYPES as f32) as u8;
                    if (new_type as usize) < NUM_TYPES {
                        store.ptype[idx] = ParticleType::from_index(new_type as usize);
                    }
                    // Slightly perturb gene expression
                    store.gene_expr[idx] = (store.gene_expr[idx]
                        + (rng.next() - 0.5) * 0.2)
                        .clamp(0.0, 1.0);
                    // Perturb epi_weight
                    store.epi_weight[idx] = (store.epi_weight[idx]
                        + (rng.next() - 0.5) * 0.1)
                        .clamp(0.0, 1.0);
                }
            }
        }

        // ── Phylogeny record ────────────────────────────────────────────
        phylogeny.add(new_oid, parent_id_for_phylo as i32, counters.tick, parent_gen + 1, child_pids.len());

        // ── Inject wave & pheromone at child position ───────────────────
        fields.wave_amp.inject(
            child_centroid.0, child_centroid.1, child_centroid.2, ws, 1.0,
        );
        fields.phero_attr.inject(
            child_centroid.0, child_centroid.1, child_centroid.2, ws, 0.5,
        );

        // ── Event & counter ─────────────────────────────────────────────
        counters.total_repro += 1;
        events.push(
            counters.tick,
            format!(
                "Org {} reproduced → child {} (gen {}, {} particles)",
                oid,
                new_oid,
                parent_gen + 1,
                child_pids.len()
            ),
            "reproduction".into(),
        );
    }

    // ── Decrement cooldowns ─────────────────────────────────────────────
    for (_oid, oinfo) in org_reg.organisms.iter_mut() {
        if oinfo.repro_cooldown > 0 {
            oinfo.repro_cooldown -= 1;
        }
    }
}

pub fn reproduce_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut events: ResMut<EventLog>,
    mut counters: ResMut<SimCounters>,
    mut fields: ResMut<SimFields>,
    mut phylogeny: ResMut<PhylogenyTree>,
    mut rng: ResMut<SimRng>,
) {
    reproduce_inner(&mut *store, &*config, &mut *org_reg, &mut *events, &mut *counters, &mut *fields, &mut *phylogeny, &mut *rng);
}
