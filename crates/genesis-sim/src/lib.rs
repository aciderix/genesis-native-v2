//! # Genesis Simulation Crate
//!
//! This crate implements the core particle-life simulation for the Genesis project.
//! It is a pure Rust / Bevy 0.15 library with no rendering — just physics, chemistry,
//! biology, ecology, and emergent behaviour.
//!
//! ## Architecture
//!
//! The simulation is driven by a single `FixedUpdate` system (`simulation_tick`) that
//! orchestrates every subsystem in a deterministic order.  Each subsystem lives in its
//! own module under `systems::` and exposes a plain-Rust `_inner` function that accepts
//! ordinary `&mut` references so the central tick can call them without going through
//! the Bevy scheduler.
//!
//! ### Tick phases (in order)
//!
//! 1. **Grid rebuild** – populate the spatial hash for neighbour queries.
//! 2. **Forces** – pairwise interactions, affinities, repulsion, pheromone gradients.
//! 3. **Integration** – Verlet / Euler integration of velocities and positions.
//! 4. **Bond formation / breaking** – create and destroy chemical bonds.
//! 5. **Signal propagation & phase** – signals travel along bonds; oscillator phase.
//! 6. **Metabolism** – energy intake, catalysis, waste, day/night cycle.
//! 7. **Organism detection** – connected-component analysis to identify organisms.
//! 8. **Advanced** – gene expression, epigenetics, specialisation.
//! 9. **Reproduction** – budding / fission of organisms (every 5 ticks).
//! 10. **Colony detection** – cluster organisms into colonies (every 10 ticks).
//! 11. **Field updates** – diffuse / decay scalar fields, inject nutrients.
//! 12. **V6 extras** – immune system, symbiogenesis, sexual reproduction, niches.
//! 13. **Symbols & tools** – symbolic communication, tool use, construction.
//! 14. **Culture & metacognition** – meme propagation, self-model updates.
//! 15. **Cleanup** – remove dead particles and compact storage (every 50 ticks).

// ── Module declarations ─────────────────────────────────────────────────────

pub mod components;
pub mod config;
pub mod resources;
pub mod particle_store;
pub mod util;
pub mod systems;
pub mod saveload;

// ── Imports ─────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use config::{
    generate_config, mulberry32, randomize_matrix, SimConfig,
    BASE_AFFINITY, BASE_BOND_STRENGTH, BASE_SIGNAL_CONDUCTANCE,
};
use particle_store::{ParticleStore, SimRng};
use resources::*;
use util::{ScalarField, SpatialGrid};

// ── Plugin ──────────────────────────────────────────────────────────────────

/// The main simulation plugin.
///
/// Add this to a Bevy `App` to get a fully self-contained particle-life
/// simulation running in `FixedUpdate`.
///
/// ```rust,no_run
/// use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
/// use genesis_sim::GenesisSimPlugin;
///
/// App::new()
///     .add_plugins(MinimalPlugins)
///     .add_plugins(GenesisSimPlugin { seed: Some(42) })
///     .run();
/// ```
pub struct GenesisSimPlugin {
    /// Optional seed for the PRNG.  `None` will pick one based on entropy.
    pub seed: Option<u32>,
}

impl Default for GenesisSimPlugin {
    fn default() -> Self {
        Self { seed: None }
    }
}

impl Plugin for GenesisSimPlugin {
    fn build(&self, app: &mut App) {
        // ── 1. Configuration ────────────────────────────────────────────
        let config = generate_config(self.seed);
        let seed = config.seed;

        // ── 2. Randomised interaction matrices ──────────────────────────
        let mut rng_fn = mulberry32(seed);
        let affinity = randomize_matrix(&BASE_AFFINITY, &mut rng_fn, 0.35);
        let bond_str = randomize_matrix(&BASE_BOND_STRENGTH, &mut rng_fn, 0.25);
        let sig_cond = randomize_matrix(&BASE_SIGNAL_CONDUCTANCE, &mut rng_fn, 0.2);
        let matrices = SimMatrices {
            affinity,
            bond_str,
            sig_cond,
        };

        // ── 3. Spatial grid ─────────────────────────────────────────────
        let grid = SpatialGrid::new(config.interaction_radius);

        // ── 4. Scalar fields ────────────────────────────────────────────
        let fields = SimFields {
            nutrient: ScalarField::new(20),
            phero_attr: ScalarField::new(14),
            phero_alarm: ScalarField::new(14),
            phero_trail: ScalarField::new(14),
            wave_amp: ScalarField::new(14),
            wave_prev: ScalarField::new(14),
            meta_a: ScalarField::new(14),
            meta_b: ScalarField::new(14),
            meta_c: ScalarField::new(14),
            symbol: std::array::from_fn(|_| ScalarField::new(14)),
        };

        // ── 5. Hydrothermal vents ───────────────────────────────────────
        let ws = config.world_size;
        let vents: Vec<Vent> = (0..config.vent_count)
            .map(|_| Vent {
                position: Vec3::new(
                    (rng_fn() - 0.5) * ws * 0.8,
                    (rng_fn() - 0.5) * ws * 0.8,
                    (rng_fn() - 0.5) * ws * 0.8,
                ),
                strength: config.vent_strength * (0.5 + rng_fn()),
                radius: 4.0 + rng_fn() * 6.0,
            })
            .collect();

        // ── 6. Initial particle population ──────────────────────────────
        let mut store = ParticleStore::default();
        let mut sim_rng = SimRng::new(seed);

        for _ in 0..config.particle_count {
            let ptype =
                ParticleStore::pick_type(&config.type_distribution, &mut sim_rng);
            let x = (sim_rng.next() - 0.5) * ws;
            let y = (sim_rng.next() - 0.5) * ws;
            let z = (sim_rng.next() - 0.5) * ws;
            let energy = 8.0 + sim_rng.next() * 12.0;
            store.spawn(ptype, x, y, z, energy, &mut sim_rng);
        }

        // ── 7. Insert all resources ─────────────────────────────────────
        app.insert_resource(config)
            .insert_resource(matrices)
            .insert_resource(grid)
            .insert_resource(fields)
            .insert_resource(VentList(vents))
            .insert_resource(store)
            .insert_resource(sim_rng)
            // Counters / stats / history
            .insert_resource(SimCounters::default())
            .insert_resource(SimStats::default())
            .insert_resource(SimHistory::default())
            .insert_resource(EventLog::default())
            // Organism / colony / phylogeny tracking
            .insert_resource(OrganismRegistry::default())
            .insert_resource(ColonyRegistry::default())
            .insert_resource(PhylogenyTree::default())
            // V5 extras
            .insert_resource(ComboState::default())
            .insert_resource(ContactTracker::default())
            .insert_resource(OrgSignatures::default())
            .insert_resource(GeneMap::default())
            // V6+ extras
            .insert_resource(ActiveSymbolCodes::default())
            .insert_resource(ActiveMemes::default())
            .insert_resource(BuildSites::default())
            .insert_resource(BuildStructureCount::default())
            .insert_resource(MetaCogOrgCount::default())
            .insert_resource(ToolGrabCount::default())
            .insert_resource(MetaboliteFlowRate::default())
            .insert_resource(ActiveGeneCount::default())
            .insert_resource(CulturalEventCount::default())
            .insert_resource(DayNightState::default())
            .insert_resource(SaveLoadRequest::default());

        // ── 8. Systems ──────────────────────────────────────────────────
        // Startup: seed the nutrient field around hydrothermal vents.
        // FixedUpdate: run the deterministic simulation tick (only when not paused).
        app.add_systems(Startup, seed_nutrients_system)
            .add_systems(
                FixedUpdate,
                simulation_tick.run_if(|config: Res<SimConfig>| !config.paused),
            )
            .add_systems(Update, handle_save_load_system);
    }
}

// ── Startup system ──────────────────────────────────────────────────────────

/// Seeds the nutrient scalar field with initial concentrations around each
/// hydrothermal vent so particles have something to metabolise on tick 0.
fn seed_nutrients_system(
    mut fields: ResMut<SimFields>,
    vents: Res<VentList>,
    config: Res<SimConfig>,
) {
    let ws = config.world_size;
    for vent in &vents.0 {
        // Inject a cluster of nutrient samples around the vent position.
        for _ in 0..20 {
            fields
                .nutrient
                .inject(vent.position.x, vent.position.y, vent.position.z, ws, 0.5);
        }
    }
}

// ── Main simulation tick ────────────────────────────────────────────────────


/// Grouped advanced resources to stay within Bevy's 16-param limit.
#[derive(SystemParam)]
struct SimAdvancedParams<'w> {
    org_sigs: ResMut<'w, OrgSignatures>,
    contacts: ResMut<'w, ContactTracker>,
    active_symbols: ResMut<'w, ActiveSymbolCodes>,
    active_genes: ResMut<'w, ActiveGeneCount>,
    cultural_count: ResMut<'w, CulturalEventCount>,
    metacog_count: ResMut<'w, MetaCogOrgCount>,
    tool_count: ResMut<'w, ToolGrabCount>,
    build_sites: ResMut<'w, BuildSites>,
    build_count: ResMut<'w, BuildStructureCount>,
}

/// The single `FixedUpdate` system that drives the entire simulation.
///
/// It dereferences every `Res` / `ResMut` and calls plain-Rust helper
/// functions (`_inner` variants) from each subsystem module so the logic is
/// composable and testable without the Bevy scheduler.
///
/// The number of sub-ticks per frame is controlled by `SimConfig::speed`
/// to allow fast-forwarding.
#[allow(clippy::too_many_arguments)]
fn simulation_tick(
    mut store: ResMut<ParticleStore>,
    mut grid: ResMut<SpatialGrid>,
    config: Res<SimConfig>,
    matrices: Res<SimMatrices>,
    vents: Res<VentList>,
    mut fields: ResMut<SimFields>,
    mut day_night: ResMut<DayNightState>,
    mut events: ResMut<EventLog>,
    mut counters: ResMut<SimCounters>,
    mut org_reg: ResMut<OrganismRegistry>,
    mut col_reg: ResMut<ColonyRegistry>,
    mut phylogeny: ResMut<PhylogenyTree>,
    mut rng: ResMut<SimRng>,
    mut stats: ResMut<SimStats>,
    mut history: ResMut<SimHistory>,
    mut adv: SimAdvancedParams,
) {
    let ticks = config.speed as u32;

    for _ in 0..ticks {
        counters.tick += 1;
        let tick = counters.tick;

        // ── Phase 1: Spatial grid rebuild ───────────────────────────────
        // Rebuilds the spatial hash so neighbour queries reflect current
        // particle positions.
        systems::grid::rebuild_grid_inner(&mut grid, &store);

        // ── Phase 2: Pairwise forces ────────────────────────────────────
        // Computes affinity/repulsion forces, pheromone-gradient forces,
        // and vent attraction/repulsion.
        systems::forces::apply_forces_inner(
            &mut store,
            &grid,
            &config,
            &matrices,
            &vents,
            &fields,
            &day_night,
            &mut rng,
        );

        // ── Phase 3: Integration ────────────────────────────────────────
        // Euler-step velocities → positions, apply damping and world-wrap.
        systems::integrate::integrate_inner(&mut store, &config);

        // ── Phase 4: Bonds ──────────────────────────────────────────────
        // Form new chemical bonds between nearby compatible particles.
        systems::bonds::form_bonds_inner(
            &mut store,
            &grid,
            &config,
            &matrices,
            &mut rng,
        );
        // Break overstressed or energy-depleted bonds.
        systems::bonds::break_bonds_inner(
            &mut store,
            &config,
            &matrices,
            &mut counters,
            &mut events,
            &mut fields,
            &org_reg,
            &mut rng,
            &stats,
        );

        // ── Phase 5: Signals & phase (every 2 ticks) ──────────────────
        if tick % 2 == 0 {
            // Propagate chemical signals along bond networks.
            systems::signals::propagate_signals_inner(&mut store, &matrices, &col_reg);
            // Advance oscillator phase for rhythmic behaviours.
            systems::signals::update_phase_inner(&mut store, &matrices);
        }

        // ── Phase 6: Metabolism ──────────────────────────────────────────
        // Energy intake from nutrients, catalytic reactions, waste
        // production, and day/night cycle advancement.
        systems::metabolism::metabolism_inner(
            &mut store,
            &config,
            &vents,
            &mut fields,
            &mut day_night,
            &mut events,
            &mut counters,
            &org_reg,
        );

        // ── Phase 7: Organism detection (every 4 ticks) ────────────────
        if tick % 4 == 0 {
            systems::organisms::detect_organisms_inner(
                &mut store,
                &mut org_reg,
                &mut events,
                &mut counters,
                &mut phylogeny,
                &stats,
            );
        }

        // ── Phase 8: Advanced / gene expression (every 2 ticks) ────────
        if tick % 2 == 0 {
            systems::advanced::advanced_systems_inner(
                &mut store,
                &mut org_reg,
                &counters,
                &mut adv.active_genes,
            );
        }

        // ── Phase 9: Reproduction (every tick) ─────────────────────────
        {
            systems::reproduction::reproduce_inner(
                &mut store,
                &config,
                &mut org_reg,
                &mut events,
                &mut counters,
                &mut fields,
                &mut phylogeny,
                &mut rng,
            );
        }

        // ── Phase 10: Colony detection (every 8 ticks) ──────────────────
        if tick % 8 == 0 {
            systems::colonies::detect_colonies_inner(
                &store,
                &config,
                &mut org_reg,
                &mut col_reg,
                &mut events,
                &counters,
            );
        }

        // ── Phase 11: Field diffusion & injection (every 4 ticks) ───────
        if tick % 4 == 0 {
            systems::fields::update_fields_inner(&mut fields, &counters, &config, &vents, &mut rng);
        }

        // ── Phase 12: V6 systems (immune, symbiogenesis, sex, niches) ───
        if tick % 6 == 0 {
            systems::v6_systems::immune_inner(
                &mut store,
                &config,
                &mut org_reg,
                &mut adv.org_sigs,
                &counters,
            );
        }
        if tick % 10 == 0 {
            systems::v6_systems::symbiogenesis_inner(
                &mut store,
                &config,
                &mut org_reg,
                &mut adv.contacts,
                &mut counters,
                &mut events,
                &mut phylogeny,
            );
        }
        if tick % 4 == 0 {
            systems::v6_systems::sexual_reproduce_inner(
                &mut store,
                &config,
                &mut org_reg,
                &mut counters,
                &mut events,
                &mut fields,
                &mut phylogeny,
                &mut rng,
            );
        }
        // Niche bonuses (every 6 ticks, matching web).
        if tick % 6 == 0 {
            systems::v6_systems::niche_bonuses_inner(&mut store, &config, &org_reg, &vents);
        }

        // ── Phase 13: Symbols & tools (every 4 ticks), construction (every 8 ticks)
        if tick % 4 == 0 {
            systems::symbols_tools::symbols_inner(
                &mut store,
                &org_reg,
                &mut fields,
                &config,
                &counters,
                &mut events,
                &mut adv.active_symbols,
                &mut rng,
            );
        }
        if tick % 5 == 0 {
            systems::symbols_tools::tool_use_inner(
                &mut store,
                &mut org_reg,
                &config,
                &mut events,
                &mut adv.tool_count,
                &mut rng,
            );
        }
        if tick % 8 == 0 {
            systems::symbols_tools::construction_inner(
                &mut store,
                &org_reg,
                &config,
                &mut adv.build_sites,
                &mut adv.build_count,
                &mut events,
                &mut rng,
            );
        }

        // ── Phase 14: Culture (every 10 ticks) & metacognition (every 8 ticks)
        if tick % 10 == 0 {
            systems::culture_metacog::culture_inner(
                &mut store,
                &mut org_reg,
                &config,
                &counters,
                &mut events,
                &mut adv.cultural_count,
                &mut fields,
                &mut rng,
            );
        }
        if tick % 8 == 0 {
            systems::culture_metacog::meta_cognition_inner(
                &mut store,
                &mut org_reg,
                &mut events,
                &counters,
                &mut adv.metacog_count,
            );
        }

        // ── Phase 15: Cleanup dead particles (every 50 ticks) ───────────
        // Compacts storage by removing particles with zero energy / flagged
        // dead.  Infrequent because it invalidates indices.
        if tick % 50 == 0 {
            store.cleanup();
        }
    }

    // ── Post-tick statistics ────────────────────────────────────────────
    update_stats(
        &store,
        &counters,
        &org_reg,
        &col_reg,
        &day_night,
        &mut stats,
        &mut history,
    );
}

// ── Statistics helper ───────────────────────────────────────────────────────

/// Recomputes summary statistics from the current simulation state and
/// appends a snapshot to the rolling history buffers.
fn update_stats(
    store: &ParticleStore,
    counters: &SimCounters,
    org_reg: &OrganismRegistry,
    col_reg: &ColonyRegistry,
    day_night: &DayNightState,
    stats: &mut SimStats,
    history: &mut SimHistory,
) {
    stats.tick = counters.tick;
    stats.particle_count = store.alive_count;

    let mut bond_count: u32 = 0;
    let mut total_energy: f32 = 0.0;
    let mut max_gen: u32 = 0;

    for i in 0..store.len() {
        if !store.alive[i] {
            continue;
        }
        bond_count += store.bonds[i].len() as u32;
        total_energy += store.energy[i];
    }

    // Every bond is stored on both endpoints, so halve the count.
    stats.bond_count = (bond_count / 2) as usize;
    stats.total_energy = total_energy;
    stats.organism_count = org_reg.organisms.len();
    stats.colony_count = col_reg.colonies.len();

    for (_, org) in &org_reg.organisms {
        if org.generation > max_gen {
            max_gen = org.generation;
        }
    }
    stats.max_generation = max_gen;
    stats.day_phase = day_night.phase;
    stats.total_reproduction = counters.total_repro;
    stats.total_predation = counters.total_pred;

    // Append to rolling history vectors.
    history.organisms.push(stats.organism_count as f32);
    history.bonds.push(stats.bond_count as f32);
    history.energy.push(total_energy);
    history.generation.push(max_gen as f32);
    history.colonies.push(stats.colony_count as f32);
}

// ── Save / Load system ───────────────────────────────────────────────────────

/// Handles save/load requests from the UI.
///
/// Runs in `Update` so it can act even when the simulation is paused.
/// On native platforms, writes/reads `genesis_save.json` in the current
/// working directory.  On WASM, save/load is a no-op (not yet supported).
fn handle_save_load_system(
    mut store: ResMut<ParticleStore>,
    mut counters: ResMut<SimCounters>,
    mut phylogeny: ResMut<PhylogenyTree>,
    mut request: ResMut<SaveLoadRequest>,
) {
    if request.save_requested {
        request.save_requested = false;
        #[cfg(not(target_family = "wasm"))]
        {
            let json = saveload::serialize_state(&store, &counters, &phylogeny);
            match std::fs::write("genesis_save.json", &json) {
                Ok(_) => {
                    let kb = json.len() / 1024;
                    request.status_message = format!("✅ Saved ({kb} KB)");
                }
                Err(e) => {
                    request.status_message = format!("❌ Save failed: {e}");
                }
            }
        }
        #[cfg(target_family = "wasm")]
        {
            request.status_message = "⚠ Save not available on web".to_string();
        }
        request.status_tick = counters.tick;
    }

    if request.load_requested {
        request.load_requested = false;
        #[cfg(not(target_family = "wasm"))]
        {
            match std::fs::read_to_string("genesis_save.json") {
                Ok(json) => {
                    match saveload::deserialize_state(
                        &json,
                        &mut store,
                        &mut counters,
                        &mut phylogeny,
                    ) {
                        Ok(_) => {
                            request.status_message =
                                format!("✅ Loaded (tick {})", counters.tick);
                        }
                        Err(e) => {
                            request.status_message = format!("❌ Load failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    request.status_message = format!("❌ File not found: {e}");
                }
            }
        }
        #[cfg(target_family = "wasm")]
        {
            request.status_message = "⚠ Load not available on web".to_string();
        }
        request.status_tick = counters.tick;
    }
}
