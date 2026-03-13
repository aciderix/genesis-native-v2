// ── colonies.rs ── Colony detection via organism proximity ──────────────────
//
// Port of `detectColonies()` from TypeScript.
// Scans for organisms whose membrane particles are close enough to form
// inter-organism bonds, then groups them into colonies using union-find.

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::ParticleStore;

use std::collections::{HashMap, HashSet};

/// Detect and maintain colonies of nearby organisms.
///
/// Process:
///   1. For each pair of organisms, check if any of their Membrane particles
///      are within `bond_distance * 3`.
///   2. Group connected organisms via union-find.
///   3. Build ColonyInfo with inter-organism membrane-membrane bonds.
///   4. Update `colony_id` on each organism.
///
/// Runs periodically (recommended: every ~10 ticks) to avoid excessive cost.
pub fn detect_colonies_inner(
    store: &ParticleStore,
    config: &SimConfig,
    org_reg: &mut OrganismRegistry,
    col_reg: &mut ColonyRegistry,
    events: &mut EventLog,
    counters: &SimCounters,
) {
    let tick = counters.tick;

    // Only run every 10 ticks to save performance
    if tick % 10 != 0 {
        return;
    }

    let bond_dist = config.bond_distance * 3.0;
    let bond_dist2 = bond_dist * bond_dist;

    // ── Collect organism membrane positions ─────────────────────────────
    // For each organism, gather positions of its Membrane (type 4) particles.
    let org_ids: Vec<u32> = org_reg.organisms.keys().copied().collect();
    let org_count = org_ids.len();

    // Map org_id → Vec of (particle_id, x, y, z) for membrane particles
    let mut org_membranes: HashMap<u32, Vec<(u32, f32, f32, f32)>> = HashMap::new();

    for &oid in &org_ids {
        if let Some(oinfo) = org_reg.organisms.get(&oid) {
            let mut membranes = Vec::new();
            for &pid in &oinfo.members {
                if let Some(&idx) = store.id_to_index.get(&pid) {
                    if store.alive[idx] && !store.is_deposit[idx] && store.ptype[idx] == ParticleType::Membrane {
                        membranes.push((pid, store.x[idx], store.y[idx], store.z[idx]));
                    }
                }
            }
            if !membranes.is_empty() {
                org_membranes.insert(oid, membranes);
            }
        }
    }

    // ── Union-Find for grouping organisms into colonies ─────────────────
    // Index-based union-find over org_ids
    let mut parent: Vec<usize> = (0..org_count).collect();
    let mut rank: Vec<usize> = vec![0; org_count];

    // Map org_id → index in org_ids
    let id_to_idx: HashMap<u32, usize> = org_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();

    // Track membrane-membrane bonds between organisms
    let mut inter_bonds: Vec<(u32, u32)> = Vec::new(); // (particle_a, particle_b)

    // ── Pairwise organism comparison ────────────────────────────────────
    let org_id_list: Vec<u32> = org_membranes.keys().copied().collect();

    for a in 0..org_id_list.len() {
        let oid_a = org_id_list[a];
        let memb_a = &org_membranes[&oid_a];

        for b in (a + 1)..org_id_list.len() {
            let oid_b = org_id_list[b];
            let memb_b = &org_membranes[&oid_b];

            // Check if any membrane pair is within bond_dist
            let mut connected = false;
            for &(pid_a, ax, ay, az) in memb_a {
                for &(pid_b, bx, by, bz) in memb_b {
                    let dx = ax - bx;
                    let dy = ay - by;
                    let dz = az - bz;
                    let d2 = dx * dx + dy * dy + dz * dz;
                    if d2 < bond_dist2 {
                        connected = true;
                        inter_bonds.push((pid_a, pid_b));
                    }
                }
                // Early exit once we know they're connected (but keep collecting bonds)
            }

            if connected {
                // Union the two organisms
                if let (Some(&ia), Some(&ib)) = (id_to_idx.get(&oid_a), id_to_idx.get(&oid_b)) {
                    union(&mut parent, &mut rank, ia, ib);
                }
            }
        }
    }

    // ── Build colonies from union-find groups ───────────────────────────
    // Old colonies (for comparison)
    let old_colony_count = col_reg.colonies.len();

    // Clear existing colonies
    col_reg.colonies.clear();

    // Group organisms by their root
    let mut root_to_orgs: HashMap<usize, Vec<u32>> = HashMap::new();
    for (i, &oid) in org_ids.iter().enumerate() {
        let root = find(&mut parent, i);
        root_to_orgs.entry(root).or_default().push(oid);
    }

    // Only form colonies from groups with 2+ organisms
    for (_root, member_oids) in root_to_orgs {
        if member_oids.len() < 2 {
            // Solo organism — clear its colony_id
            for &oid in &member_oids {
                if let Some(oinfo) = org_reg.organisms.get_mut(&oid) {
                    oinfo.colony_id = -1;
                }
            }
            continue;
        }

        let colony_id = col_reg.next_id;
        col_reg.next_id += 1;

        // Collect inter-bonds relevant to this colony
        let member_set: HashSet<u32> = member_oids.iter().copied().collect();
        let colony_bonds: Vec<(u32, u32)> = inter_bonds
            .iter()
            .copied()
            .filter(|&(pa, pb)| {
                // Check if both particles belong to organisms in this colony
                let org_a = particle_organism(&store, pa);
                let org_b = particle_organism(&store, pb);
                match (org_a, org_b) {
                    (Some(oa), Some(ob)) => member_set.contains(&oa) && member_set.contains(&ob),
                    _ => false,
                }
            })
            .collect();

        let colony_info = ColonyInfo {
            organism_ids: member_set.clone(),
            bonds: colony_bonds,
            age: 0, // will accumulate
        };
        col_reg.colonies.insert(colony_id, colony_info);

        // Update colony_id on each member organism
        for &oid in &member_oids {
            if let Some(oinfo) = org_reg.organisms.get_mut(&oid) {
                oinfo.colony_id = colony_id as i32;
            }
        }
    }

    // ── Emit events for new colonies ────────────────────────────────────
    let new_colony_count = col_reg.colonies.len();
    if new_colony_count > old_colony_count {
        events.push(
            tick,
            format!(
                "Colony count increased: {} → {} ({} colonies)",
                old_colony_count, new_colony_count, new_colony_count
            ),
            "colony".into(),
        );
    }

    // ── Age existing colonies ───────────────────────────────────────────
    for (_cid, cinfo) in col_reg.colonies.iter_mut() {
        cinfo.age += 1;
    }
}

pub fn detect_colonies_system(
    store: Res<ParticleStore>,
    config: Res<SimConfig>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut col_reg: ResMut<ColonyRegistry>,
    mut events: ResMut<EventLog>,
    counters: Res<SimCounters>,
) {
    detect_colonies_inner(&*store, &*config, &mut *org_reg, &mut *col_reg, &mut *events, &*counters);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Find the root of element `i` in the union-find (with path compression).
fn find(parent: &mut Vec<usize>, i: usize) -> usize {
    if parent[i] != i {
        parent[i] = find(parent, parent[i]);
    }
    parent[i]
}

/// Union two elements by rank.
fn union(parent: &mut Vec<usize>, rank: &mut Vec<usize>, a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra == rb {
        return;
    }
    if rank[ra] < rank[rb] {
        parent[ra] = rb;
    } else if rank[ra] > rank[rb] {
        parent[rb] = ra;
    } else {
        parent[rb] = ra;
        rank[ra] += 1;
    }
}

/// Look up which organism a particle belongs to (by organism_id field).
fn particle_organism(store: &ParticleStore, pid: u32) -> Option<u32> {
    if let Some(&idx) = store.id_to_index.get(&pid) {
        let oid = store.organism_id[idx];
        if oid >= 0 {
            Some(oid as u32)
        } else {
            None
        }
    } else {
        None
    }
}
