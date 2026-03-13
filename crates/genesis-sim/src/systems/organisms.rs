//! Organism detection system.
//!
//! Ports the TypeScript `detectOrganisms()` method to Rust.
//!
//! Uses BFS (Breadth-First Search) through bond connectivity to discover
//! connected components of particles. A connected component of size >= 2
//! is registered as an organism.
//!
//! ## Algorithm
//!
//! 1. Mark all particles as unvisited
//! 2. For each alive, non-deposit, unvisited particle:
//!    a. If no bonds: set `organism_id = -1`, continue
//!    b. BFS through bonds to find the connected component
//!    c. If component size >= 2: register as an organism
//!    d. If component was previously part of a known organism, inherit generation
//!    e. Otherwise, start at generation 0
//! 3. Remove organisms that no longer exist
//! 4. Update `OrganismRegistry` with new/updated organisms
//! 5. Emit events for new organisms and dissolved organisms

use bevy::prelude::*;
use std::collections::{HashSet, VecDeque, HashMap};
use crate::particle_store::ParticleStore;
use crate::resources::*;
use crate::components::{ParticleType, CellRole};

/// Minimum number of particles for a group to be considered an organism.
const MIN_ORGANISM_SIZE: usize = 2;

/// Detect organisms by finding connected components through bond connectivity.
///
/// ## Steps
///
/// 1. **BFS connected components**: Walk through all alive non-deposit particles,
///    finding groups connected by bonds.
///
/// 2. **Organism lifecycle**:
///    - New components of size >= 2 → create new organism
///    - Existing organisms that match → update
///    - Organisms whose members are gone → dissolve
///
/// 3. **Generation tracking**: If members of a new component were previously
///    part of a known organism, the new organism inherits that generation + 1
///    (if it's a split/reproduction event) or the same generation (if it's the
///    same organism continuing).
///
/// 4. **Cell role assignment**: Particles in organisms are assigned roles based
///    on their type and position within the organism.
pub fn detect_organisms_inner(
    store: &mut ParticleStore,
    org_reg: &mut OrganismRegistry,
    events: &mut EventLog,
    counters: &mut SimCounters,
    phylogeny: &mut PhylogenyTree,
    stats: &SimStats,
) {
    let n = store.len();
    if n == 0 {
        return;
    }

    // -----------------------------------------------------------------------
    // Phase 1: Find connected components via BFS
    // -----------------------------------------------------------------------

    let mut visited = vec![false; n];
    // Component membership: maps particle index -> component ID
    let mut component_id = vec![-1i32; n];
    // List of components: each component is a Vec of particle indices
    let mut components: Vec<Vec<usize>> = Vec::new();
    // BFS queue
    let mut queue: VecDeque<usize> = VecDeque::new();

    for start in 0..n {
        if visited[start] || !store.alive[start] || store.is_deposit[start] {
            continue;
        }

        // Start a new BFS from this particle
        let comp_idx = components.len() as i32;
        let mut component: Vec<usize> = Vec::new();

        queue.clear();
        queue.push_back(start);
        visited[start] = true;

        while let Some(current) = queue.pop_front() {
            component.push(current);
            component_id[current] = comp_idx;

            // Explore bond neighbors
            // Collect partner IDs first to avoid borrow issues
            let partner_ids: Vec<u32> = store.bonds[current].iter().copied().collect();

            for partner_id in partner_ids {
                if let Some(partner_idx) = store.idx(partner_id) {
                    if !visited[partner_idx]
                        && store.alive[partner_idx]
                        && !store.is_deposit[partner_idx]
                    {
                        visited[partner_idx] = true;
                        queue.push_back(partner_idx);
                    }
                }
            }
        }

        components.push(component);
    }

    // Also mark unbonded particles (visited but single) as no-organism
    for i in 0..n {
        if store.alive[i] && !store.is_deposit[i] && store.bonds[i].is_empty() {
            component_id[i] = -1;
        }
    }

    // -----------------------------------------------------------------------
    // Phase 2: Match components to existing organisms
    // -----------------------------------------------------------------------

    // Track which existing organism IDs we've seen this tick
    let mut seen_org_ids: HashSet<u32> = HashSet::new();

    // For each component, check if its members already belong to an organism
    // and decide whether to keep, create new, or merge.
    //
    // Strategy: For each component, find the most common existing organism_id
    // among its members. If that organism still exists, reuse it. Otherwise,
    // create a new one.
    let mut new_org_assignments: Vec<(Vec<usize>, u32)> = Vec::new();

    for component in &components {
        if component.len() < MIN_ORGANISM_SIZE {
            // Too small to be an organism — clear any existing assignment
            for &idx in component {
                store.organism_id[idx] = -1;
                store.cell_role[idx] = CellRole::None;
            }
            continue;
        }

        // Count existing organism membership
        let mut org_votes: HashMap<u32, usize> = HashMap::new();
        let mut max_generation: u32 = 0;
        let mut parent_org_id: i32 = -1;

        for &idx in component {
            let oid = store.organism_id[idx];
            if oid >= 0 {
                let oid_u = oid as u32;
                *org_votes.entry(oid_u).or_insert(0) += 1;
                if let Some(org) = org_reg.get(oid_u) {
                    max_generation = max_generation.max(org.generation);
                    parent_org_id = oid as i32;
                }
            }
        }

        // Find the most popular existing organism ID in this component
        let best_existing: Option<u32> = org_votes
            .iter()
            .max_by_key(|&(_, &count)| count)
            .map(|(&id, _)| id);

        // Decide: reuse existing organism or create a new one
        let org_id = if let Some(existing_id) = best_existing {
            if !seen_org_ids.contains(&existing_id) {
                // This is the same organism continuing — reuse it
                seen_org_ids.insert(existing_id);
                existing_id
            } else {
                // This existing_id was already claimed by another component
                // This means the organism has split! Create a new one with generation+1
                let new_id = org_reg.create();
                if let Some(org) = org_reg.get_mut(new_id) {
                    org.generation = max_generation + 1;
                    org.parent_id = parent_org_id;
                }
                seen_org_ids.insert(new_id);

                // Record in phylogeny tree
                phylogeny.add(
                    new_id,
                    parent_org_id,
                    stats.tick,
                    max_generation + 1,
                    component.len(),
                );

                events.push(
                    stats.tick,
                    format!(
                        "Organism #{} split → new #{} (gen {}, size {})",
                        parent_org_id,
                        new_id,
                        max_generation + 1,
                        component.len(),
                    ),
                    EventType::Reproduction,
                );
                counters.total_repro += 1;

                new_id
            }
        } else {
            // Entirely new organism — no members had prior org assignment
            let new_id = org_reg.create();
            if let Some(org) = org_reg.get_mut(new_id) {
                org.generation = 0;
                org.parent_id = -1;
            }
            seen_org_ids.insert(new_id);

            // Record in phylogeny tree
            phylogeny.add(new_id, -1, stats.tick, 0, component.len());

            events.push(
                stats.tick,
                format!(
                    "New organism #{} formed (size {})",
                    new_id,
                    component.len(),
                ),
                EventType::Organism,
            );

            new_id
        };

        new_org_assignments.push((component.clone(), org_id));
    }

    // -----------------------------------------------------------------------
    // Phase 3: Apply organism assignments and update registry
    // -----------------------------------------------------------------------

    for (component, org_id) in &new_org_assignments {
        // Update particle organism membership
        for &idx in component {
            store.organism_id[idx] = *org_id as i32;
        }

        // Update organism info in the registry
        if let Some(org) = org_reg.get_mut(*org_id) {
            org.members.clear();
            org.energy = 0.0;
            org.cells.clear();

            let mut total_energy = 0.0;
            let mut min_age = u32::MAX;

            for &idx in component {
                let pid = store.id[idx];
                org.members.insert(pid);
                total_energy += store.energy[idx];
                min_age = min_age.min(store.age[idx]);

                // Track cells by role
                let role = store.cell_role[idx];
                if role != CellRole::None {
                    org.cells
                        .entry(role.as_index() as u8)
                        .or_insert_with(HashSet::new)
                        .insert(pid);
                }
            }

            org.energy = total_energy;
            // Age = ticks since first formation (approximate from member ages)
            org.age = org.age.saturating_add(1);

            // Compute fitness: composite of size, age, energy, generation
            let size_factor = (component.len() as f32).sqrt();
            let age_factor = (org.age as f32 * 0.001).min(2.0);
            let energy_factor = (total_energy * 0.1).min(3.0);
            let gen_factor = (org.generation as f32 * 0.5).min(3.0);
            org.fitness = size_factor + age_factor + energy_factor + gen_factor;

            // Check for multicellularity
            org.is_multicellular = component.len() >= 4
                && org.cells.len() >= 2;

            // Compute specialization score
            if !org.cells.is_empty() {
                let total_specialized: usize = org.cells.values().map(|s| s.len()).sum();
                org.specialization =
                    (total_specialized as f32 / component.len() as f32).min(1.0);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Phase 4: Assign cell roles to organism members
    // -----------------------------------------------------------------------

    for (component, _org_id) in &new_org_assignments {
        if component.len() < MIN_ORGANISM_SIZE {
            continue;
        }

        assign_cell_roles(&mut store, component);
    }

    // -----------------------------------------------------------------------
    // Phase 5: Remove dissolved organisms
    // -----------------------------------------------------------------------

    let all_org_ids: Vec<u32> = org_reg.organisms.keys().copied().collect();
    for org_id in all_org_ids {
        if !seen_org_ids.contains(&org_id) {
            // This organism no longer exists — all its members are gone or dispersed
            if let Some(info) = org_reg.remove(org_id) {
                if info.members.len() >= MIN_ORGANISM_SIZE {
                    events.push(
                        stats.tick,
                        format!(
                            "Organism #{} dissolved (was gen {}, size {}, fitness {:.1})",
                            org_id,
                            info.generation,
                            info.members.len(),
                            info.fitness,
                        ),
                        EventType::Death,
                    );
                }
            }
        }
    }
}

pub fn detect_organisms_system(
    mut store: ResMut<ParticleStore>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut events: ResMut<EventLog>,
    mut counters: ResMut<SimCounters>,
    mut phylogeny: ResMut<PhylogenyTree>,
    stats: Res<SimStats>,
) {
    detect_organisms_inner(&mut *store, &mut *org_reg, &mut *events, &mut *counters, &mut *phylogeny, &*stats);
}

// ---------------------------------------------------------------------------
// Cell role assignment
// ---------------------------------------------------------------------------

/// Assign specialised cell roles to particles within an organism.
///
/// Role assignment is based on particle type and (simplified) position:
///
/// - **Catalyst** → `Sensor` (detects nutrients, signals)
/// - **Motor** → `MotorCell` (drives locomotion)
/// - **Membrane** → `Defense` (protects the organism)
/// - **Beta** → `Digester` (energy processing)
/// - **Data** → `Reproducer` (carries genetic information for replication)
/// - **Alpha** → `None` (structural, no special role)
///
/// Only organisms with >= 3 members get role assignments. Single-type
/// organisms don't specialise.
fn assign_cell_roles(store: &mut ParticleStore, component: &[usize]) {
    if component.len() < 3 {
        // Too small for meaningful specialisation
        for &idx in component {
            store.cell_role[idx] = CellRole::None;
        }
        return;
    }

    // Count particle types in the organism
    let mut type_counts = [0usize; 6];
    for &idx in component {
        type_counts[store.ptype[idx].as_index()] += 1;
    }

    // Only assign roles if there's type diversity (at least 2 different types)
    let type_diversity = type_counts.iter().filter(|&&c| c > 0).count();
    if type_diversity < 2 {
        for &idx in component {
            store.cell_role[idx] = CellRole::None;
        }
        return;
    }

    // Assign roles based on particle type
    for &idx in component {
        store.cell_role[idx] = match store.ptype[idx] {
            ParticleType::Catalyst => CellRole::Sensor,
            ParticleType::Motor    => CellRole::MotorCell,
            ParticleType::Membrane => CellRole::Defense,
            ParticleType::Beta     => CellRole::Digester,
            ParticleType::Data     => CellRole::Reproducer,
            ParticleType::Alpha    => CellRole::None,
        };
    }
}
