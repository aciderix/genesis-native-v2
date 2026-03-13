// ── symbols_tools.rs ── Symbolic Communication, Tool Use, Construction ──────
//
// This module ports three advanced behavioral subsystems:
//   P3.4 — Symbol Communication: organisms emit/read symbols on field channels
//   P4.1 — Tool Use:             Motor particles grab and use deposit "tools"
//   P4.2 — Construction:         organisms build structures from deposits

use bevy::prelude::*;
use crate::components::*;
use crate::config::SimConfig;
use crate::resources::*;
use crate::particle_store::{ParticleStore, SimRng};

use std::collections::HashSet;

// ─── P3.4: Symbol Communication ─────────────────────────────────────────────
//
// Organisms with Catalyst particles (Sensors) and >3 members can emit symbols
// onto the 8 symbol field channels.  The channel is chosen based on
// environmental context (vents, predators, energy level).
//
// Organisms with Data particles can *read* symbols at their centroid and
// respond: food → attract, danger → flee, mating → boost repro readiness.

pub fn symbols_inner(
    store: &mut ParticleStore,
    org_reg: &OrganismRegistry,
    fields: &mut SimFields,
    config: &SimConfig,
    counters: &SimCounters,
    events: &mut EventLog,
    active_symbols: &mut ActiveSymbolCodes,
    rng: &mut SimRng,
) {
    let ws = config.world_size;
    let tick = counters.tick;

    active_symbols.0.clear();

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
        if size < 4 {
            continue; // Need >3 members
        }

        // ── Compute organism centroid ──────────────────────────────────────
        let (mut cx, mut cy, mut cz) = (0.0f32, 0.0f32, 0.0f32);
        for &idx in &member_indices {
            cx += store.x[idx];
            cy += store.y[idx];
            cz += store.z[idx];
        }
        cx /= size as f32;
        cy /= size as f32;
        cz /= size as f32;

        // ── Count types ───────────────────────────────────────────────────
        let mut type_counts = [0u32; NUM_TYPES];
        for &idx in &member_indices {
            type_counts[store.ptype[idx].as_index()] += 1;
        }
        let has_catalyst = type_counts[ParticleType::Catalyst.as_index()] > 0;
        let has_data = type_counts[ParticleType::Data.as_index()] > 0;

        // ── EMIT symbols ──────────────────────────────────────────────────
        // Catalyst particles (Sensors) with signal > 0.5 emit symbols
        if has_catalyst {
            for &idx in &member_indices {
                if store.ptype[idx] != ParticleType::Catalyst {
                    continue;
                }
                if store.signal[idx] <= 0.5 {
                    continue;
                }

                // Choose symbol channel based on environmental context
                let channel: u8 = if is_near_vent(&store, idx, ws) {
                    1 // Food source
                } else if oinfo.predation_count > 3 {
                    2 // Danger
                } else if oinfo.energy > size as f32 * 3.0 {
                    3 // Mating / high energy
                } else {
                    // Default: channel based on dominant type
                    let dominant = type_counts
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, &c)| c)
                        .map(|(t, _)| t)
                        .unwrap_or(0);
                    (dominant as u8 % 7) + 1 // Channels 1-7
                };

                // Clamp channel to [1, 7] (index 0-6 in symbol array which has 8 slots)
                let ch = channel.clamp(1, 7) as usize;

                // Emit symbol energy at particle position
                fields.symbol[ch].inject(
                    store.x[idx],
                    store.y[idx],
                    store.z[idx],
                    ws,
                    0.3,
                );

                // Mark the particle's symbol code
                store.symbol_code[idx] = channel;
                active_symbols.0.insert(channel);
            }
        }

        // ── READ symbols ──────────────────────────────────────────────────
        // Data particles sense symbol fields at the organism centroid
        if has_data {
            // Sample all symbol channels at the centroid
            let mut strongest_channel: u8 = 0;
            let mut strongest_value: f32 = 0.15; // Threshold

            for ch in 0..8 {
                let val = fields.symbol[ch].sample(cx, cy, cz, ws);
                if val > strongest_value {
                    strongest_value = val;
                    strongest_channel = ch as u8;
                }
            }

            if strongest_channel > 0 {
                // Apply behavioral response based on symbol meaning
                match strongest_channel {
                    1 => {
                        // Food symbol: attract toward gradient
                        let grad = fields.symbol[1].gradient(cx, cy, cz, ws);
                        if grad.length_squared() > 0.0001 {
                            let dir = grad.normalize() * 0.03;
                            for &idx in &member_indices {
                                store.vx[idx] += dir.x;
                                store.vy[idx] += dir.y;
                                store.vz[idx] += dir.z;
                            }
                        }
                    }
                    2 => {
                        // Danger symbol: flee from gradient source
                        let grad = fields.symbol[2].gradient(cx, cy, cz, ws);
                        if grad.length_squared() > 0.0001 {
                            let dir = grad.normalize() * -0.04; // Flee
                            for &idx in &member_indices {
                                store.vx[idx] += dir.x;
                                store.vy[idx] += dir.y;
                                store.vz[idx] += dir.z;
                            }
                        }
                    }
                    3 => {
                        // Mating symbol: boost reproduction readiness
                        // (Signal boost on Data particles to trigger repro sooner)
                        for &idx in &member_indices {
                            if store.ptype[idx] == ParticleType::Data {
                                store.signal[idx] =
                                    (store.signal[idx] + 0.02).clamp(-1.0, 1.0);
                            }
                        }
                    }
                    _ => {
                        // Generic attraction toward symbol source
                        let ch = strongest_channel as usize;
                        if ch < 8 {
                            let grad = fields.symbol[ch].gradient(cx, cy, cz, ws);
                            if grad.length_squared() > 0.0001 {
                                let dir = grad.normalize() * 0.02;
                                for &idx in &member_indices {
                                    store.vx[idx] += dir.x;
                                    store.vy[idx] += dir.y;
                                    store.vz[idx] += dir.z;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn symbols_system(
    mut store: ResMut<ParticleStore>,
    org_reg: Res<OrganismRegistry>,
    mut fields: ResMut<SimFields>,
    config: Res<SimConfig>,
    counters: Res<SimCounters>,
    mut events: ResMut<EventLog>,
    mut active_symbols: ResMut<ActiveSymbolCodes>,
    mut rng: ResMut<SimRng>,
) {
    symbols_inner(&mut *store, &*org_reg, &mut *fields, &*config, &*counters, &mut *events, &mut *active_symbols, &mut *rng);
}

/// Helper: check if a particle is near any vent (rough heuristic using
/// nutrient field density as a proxy).
fn is_near_vent(store: &ParticleStore, idx: usize, _ws: f32) -> bool {
    // Heuristic: particles near vents tend to have high energy and be near
    // the bottom of the world (vents are typically placed low).
    // More accurate would check vent positions, but we use a simple proxy.
    store.energy[idx] > 5.0 && store.y[idx] < 0.0
}

// ─── P4.1: Tool Use ────────────────────────────────────────────────────────
//
// Motor particles in organisms with prior tool experience (tool_use_count > 0)
// or meta-cognitive depth can grab nearby deposits as "tools".
//
// Tool usage: Motor holding a tool near an enemy gets extra damage potential.
// Release: energy < 1 or random 1% chance per tick.

pub fn tool_use_inner(
    store: &mut ParticleStore,
    org_reg: &mut OrganismRegistry,
    config: &SimConfig,
    events: &mut EventLog,
    tool_count: &mut ToolGrabCount,
    rng: &mut SimRng,
) {
    let ws = config.world_size;
    let bond_dist = config.bond_distance;
    let grab_range = bond_dist * 2.0;
    let grab_range_sq = grab_range * grab_range;
    let tick = 0u64; // We don't need tick for tool logic, but keep consistent

    let len = store.id.len();

    // Collect deposit indices for proximity checks
    let deposit_indices: Vec<usize> = (0..len)
        .filter(|&i| store.alive[i] && store.is_deposit[i])
        .collect();

    // Process Motor particles
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }
        if store.ptype[i] != ParticleType::Motor {
            continue;
        }
        if store.organism_id[i] < 0 {
            continue;
        }

        let oid = store.organism_id[i] as u32;

        // Check org eligibility: tool_use_count > 0 or meta_cog_depth > 0
        let (tool_use_count, meta_depth) = match org_reg.organisms.get(&oid) {
            Some(o) => (o.tool_use_count, o.meta_cog_depth),
            None => continue,
        };

        if tool_use_count == 0 && meta_depth == 0.0 {
            // Small base probability for first tool use
            if rng.next() > 0.001 {
                continue;
            }
        }

        let currently_holding = store.held_tool[i] >= 0;

        if !currently_holding {
            // ── Try to GRAB a nearby deposit ──────────────────────────────
            let grab_prob = 0.02 * (1.0 + tool_use_count as f32 * 0.3);
            if rng.next() > grab_prob {
                continue;
            }

            // Find nearest deposit within grab range
            let mut nearest_dist = f32::MAX;
            let mut nearest_dep: Option<usize> = None;

            for &di in &deposit_indices {
                // Skip deposits already held by someone
                // (A deposit is "held" if any motor points to it)
                // Simple check: just grab any nearby deposit
                let dsq = store.distance_sq_wrapped(i, di, ws);
                if dsq < grab_range_sq && dsq < nearest_dist {
                    nearest_dist = dsq;
                    nearest_dep = Some(di);
                }
            }

            if let Some(di) = nearest_dep {
                let dep_id = store.id[di] as i32;
                store.held_tool[i] = dep_id;

                // Increment org tool use count
                if let Some(oinfo) = org_reg.organisms.get_mut(&oid) {
                    oinfo.tool_use_count += 1;
                }

                tool_count.0 += 1;
            }
        } else {
            // ── Release conditions ────────────────────────────────────────
            let should_release = store.energy[i] < 1.0 || rng.next() < 0.01;

            if should_release {
                store.held_tool[i] = -1;
            } else {
                // ── Tool usage: held tool gives energy/combat bonus ───────
                // Motor with tool gets a small energy bonus representing
                // enhanced foraging capability
                store.energy[i] += 0.003;
            }
        }
    }
}

pub fn tool_use_system(
    mut store: ResMut<ParticleStore>,
    mut org_reg: ResMut<OrganismRegistry>,
    config: Res<SimConfig>,
    mut events: ResMut<EventLog>,
    mut tool_count: ResMut<ToolGrabCount>,
    mut rng: ResMut<SimRng>,
) {
    tool_use_inner(&mut *store, &mut *org_reg, &*config, &mut *events, &mut *tool_count, &mut *rng);
}

// ─── P4.2: Intentional Construction ────────────────────────────────────────
//
// Organisms with build_score > 0 or with Motor + Alpha + Membrane composition
// can construct structures.  Motor particles with held tools deposit them at
// quantized grid positions when signal > 0.5.  When 3+ deposits accumulate
// at the same site, a "structure" is formed that provides area energy bonuses.

pub fn construction_inner(
    store: &mut ParticleStore,
    org_reg: &OrganismRegistry,
    config: &SimConfig,
    build_sites: &mut BuildSites,
    build_count: &mut BuildStructureCount,
    events: &mut EventLog,
    rng: &mut SimRng,
) {
    let ws = config.world_size;
    let len = store.id.len();

    // ── Phase 1: Motor particles attempt to deposit at build sites ────────
    for i in 0..len {
        if !store.alive[i] || store.is_deposit[i] {
            continue;
        }
        if store.ptype[i] != ParticleType::Motor {
            continue;
        }
        if store.organism_id[i] < 0 {
            continue;
        }

        let oid = store.organism_id[i] as u32;

        // Check eligibility: org has build_score or Motor+Alpha+Membrane composition
        let eligible = match org_reg.organisms.get(&oid) {
            Some(o) => {
                if o.build_score > 0.0 {
                    true
                } else {
                    // Check for Motor + Alpha + Membrane composition
                    let mut has_motor = false;
                    let mut has_alpha = false;
                    let mut has_membrane = false;
                    for &pid in &o.members {
                        if let Some(&idx) = store.id_to_index.get(&pid) {
                            if store.alive[idx] && !store.is_deposit[idx] {
                                match store.ptype[idx] {
                                    ParticleType::Motor => has_motor = true,
                                    ParticleType::Alpha => has_alpha = true,
                                    ParticleType::Membrane => has_membrane = true,
                                    _ => {}
                                }
                            }
                        }
                    }
                    has_motor && has_alpha && has_membrane
                }
            }
            None => false,
        };

        if !eligible {
            continue;
        }

        // Must be holding a tool and have signal > 0.5
        if store.held_tool[i] < 0 || store.signal[i] <= 0.5 {
            continue;
        }

        // Low probability per tick of actually placing
        if rng.next() > 0.05 {
            continue;
        }

        // Quantize position to grid cell (resolution ~2 units)
        let gx = (store.x[i] / 2.0).floor() as i32;
        let gy = (store.y[i] / 2.0).floor() as i32;
        let gz = (store.z[i] / 2.0).floor() as i32;

        // Deposit the held tool at this site
        store.held_tool[i] = -1; // Release tool

        let site_count = build_sites.0.entry((gx, gy, gz)).or_insert(0);
        *site_count += 1;

        // Check if a structure has formed (3+ deposits at same site)
        if *site_count == 3 {
            build_count.0 += 1;

            // Update org build score
            if let Some(oinfo) = org_reg.organisms.get(&oid) {
                // Note: we'd need mut org_reg for this, but since construction
                // is primarily tracked via BuildSites, we emit an event instead
            }

            events.push(
                0, // tick not available here directly; parent system provides it
                format!(
                    "Structure formed at ({}, {}, {}) — {} total structures",
                    gx, gy, gz, build_count.0
                ),
                EventType::Build,
            );
        }
    }

    // ── Phase 2: Structures provide area energy bonus ─────────────────────
    // For each completed structure (count >= 3), give energy to nearby particles
    let structure_sites: Vec<(i32, i32, i32)> = build_sites
        .0
        .iter()
        .filter(|(_, &count)| count >= 3)
        .map(|(&pos, _)| pos)
        .collect();

    let bonus_range_sq = 4.0 * 4.0; // 4-unit radius
    let bonus_per_tick = 0.005;

    for (gx, gy, gz) in &structure_sites {
        // Convert quantized grid back to world position (center of cell)
        let sx = *gx as f32 * 2.0 + 1.0;
        let sy = *gy as f32 * 2.0 + 1.0;
        let sz = *gz as f32 * 2.0 + 1.0;

        for i in 0..len {
            if !store.alive[i] || store.is_deposit[i] {
                continue;
            }
            // Only benefit organisms
            if store.organism_id[i] < 0 {
                continue;
            }

            let dx = store.x[i] - sx;
            let dy = store.y[i] - sy;
            let dz = store.z[i] - sz;
            let dsq = dx * dx + dy * dy + dz * dz;

            if dsq < bonus_range_sq {
                store.energy[i] += bonus_per_tick;
            }
        }
    }
}

pub fn construction_system(
    mut store: ResMut<ParticleStore>,
    org_reg: Res<OrganismRegistry>,
    config: Res<SimConfig>,
    mut build_sites: ResMut<BuildSites>,
    mut build_count: ResMut<BuildStructureCount>,
    mut events: ResMut<EventLog>,
    mut rng: ResMut<SimRng>,
) {
    construction_inner(&mut *store, &*org_reg, &*config, &mut *build_sites, &mut *build_count, &mut *events, &mut *rng);
}
