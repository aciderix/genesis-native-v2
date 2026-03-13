//! Shared Bevy Resources for the Genesis Engine simulation.
//!
//! This module houses every `Resource` that systems read from or write to
//! during simulation ticks: statistics, registries, spatial data structures,
//! event logs, and miscellaneous global counters.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

// ---------------------------------------------------------------------------
// Interaction matrices (per-universe, randomised from base)
// ---------------------------------------------------------------------------

/// The three 6×6 interaction matrices that govern particle-particle forces,
/// bond strengths, and signal conductance.  Generated once per universe from
/// the base matrices + a seed.
#[derive(Resource, Clone, Debug)]
pub struct SimMatrices {
    /// Attraction / repulsion strength between type pairs.
    pub affinity: [[f32; 6]; 6],
    /// Bond strength between type pairs.
    pub bond_str: [[f32; 6]; 6],
    /// Signal conductance across bonds between type pairs.
    pub sig_cond: [[f32; 6]; 6],
}

impl Default for SimMatrices {
    fn default() -> Self {
        use crate::config::*;
        Self {
            affinity: BASE_AFFINITY,
            bond_str: BASE_BOND_STRENGTH,
            sig_cond: BASE_SIGNAL_CONDUCTANCE,
        }
    }
}

// ---------------------------------------------------------------------------
// Simulation era
// ---------------------------------------------------------------------------

/// The current evolutionary era of the simulation.  The simulation
/// automatically advances through eras as complexity milestones are reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum SimEra {
    #[default]
    Primordial   = 0,
    Chemical     = 1,
    ProtoLife     = 2,
    Replication  = 3,
    Predatory    = 4,
    Colonial     = 5,
    Metabolic    = 6,
    Genetic      = 7,
    Specialized  = 8,
    Symbolic     = 9,
    Tool         = 10,
    Construction = 11,
    Cultural     = 12,
    Cognitive    = 13,
    Symbiotic    = 14,
}

impl SimEra {
    /// Total number of eras.
    pub const COUNT: usize = 15;

    pub fn as_index(self) -> usize {
        self as usize
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0  => Self::Primordial,
            1  => Self::Chemical,
            2  => Self::ProtoLife,
            3  => Self::Replication,
            4  => Self::Predatory,
            5  => Self::Colonial,
            6  => Self::Metabolic,
            7  => Self::Genetic,
            8  => Self::Specialized,
            9  => Self::Symbolic,
            10 => Self::Tool,
            11 => Self::Construction,
            12 => Self::Cultural,
            13 => Self::Cognitive,
            14 => Self::Symbiotic,
            _  => Self::Primordial,
        }
    }

    /// Human-readable name of this era.
    pub fn name(self) -> &'static str {
        match self {
            Self::Primordial   => "Primordial",
            Self::Chemical     => "Chemical",
            Self::ProtoLife     => "Proto-Life",
            Self::Replication  => "Replication",
            Self::Predatory    => "Predatory",
            Self::Colonial     => "Colonial",
            Self::Metabolic    => "Metabolic",
            Self::Genetic      => "Genetic",
            Self::Specialized  => "Specialized",
            Self::Symbolic     => "Symbolic",
            Self::Tool         => "Tool",
            Self::Construction => "Construction",
            Self::Cultural     => "Cultural",
            Self::Cognitive    => "Cognitive",
            Self::Symbiotic    => "Symbiotic",
        }
    }
}

// ---------------------------------------------------------------------------
// Circular buffer (fixed capacity, used for history tracking)
// ---------------------------------------------------------------------------

/// A simple fixed-capacity circular buffer backed by a `Vec<T>`.
///
/// When the buffer is full, new values overwrite the oldest entry.
/// The generic parameter `N` is the maximum capacity.
#[derive(Debug, Clone)]
pub struct CircularBuffer<T, const N: usize> {
    data: Vec<T>,
    head: usize,
    len: usize,
}

impl<T: Default + Clone, const N: usize> Default for CircularBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default + Clone, const N: usize> CircularBuffer<T, N> {
    /// Create a new empty circular buffer.
    pub fn new() -> Self {
        Self {
            data: vec![T::default(); N],
            head: 0,
            len: 0,
        }
    }

    /// Push a value, overwriting the oldest if full.
    pub fn push(&mut self, value: T) {
        self.data[self.head] = value;
        self.head = (self.head + 1) % N;
        if self.len < N {
            self.len += 1;
        }
    }

    /// Return the most-recently pushed value, if any.
    pub fn last(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        let idx = if self.head == 0 { N - 1 } else { self.head - 1 };
        Some(&self.data[idx])
    }

    /// Iterate from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let start = if self.len < N { 0 } else { self.head };
        let len = self.len;
        let data = &self.data;
        (0..len).map(move |i| &data[(start + i) % N])
    }

    /// Current number of stored elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Is the buffer empty?
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Maximum capacity.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Return the buffer contents as a contiguous slice (oldest to newest).
    /// Note: allocates a Vec internally because the ring buffer may wrap.
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Simulation statistics
// ---------------------------------------------------------------------------

/// Aggregate statistics about the simulation state, updated each tick.
#[derive(Resource, Clone, Debug)]
pub struct SimStats {
    /// Current simulation tick.
    pub tick: u64,
    /// Number of live particles.
    pub particle_count: usize,
    /// Total number of active bonds.
    pub bond_count: usize,
    /// Number of registered organisms.
    pub organism_count: usize,
    /// Number of registered colonies.
    pub colony_count: usize,
    /// Sum of all particle energies.
    pub total_energy: f32,
    /// Highest generation number among organisms.
    pub max_generation: u32,
    /// Current evolutionary era.
    pub era: SimEra,
    /// Day/night cycle phase (0.0 – 1.0).
    pub day_phase: f32,
    /// Measured ticks executed per wall-clock second.
    pub ticks_per_second: f32,
    /// Measured wall-clock milliseconds per tick.
    pub ms_per_tick: f32,
    /// Number of deposit entities.
    pub deposit_count: usize,
    /// Average organism fitness.
    pub avg_fitness: f32,
    /// Highest organism fitness.
    pub max_fitness: f32,
    /// Total predation events since simulation start.
    pub total_predation: u64,
    /// Total reproduction events since simulation start.
    pub total_reproduction: u64,
    /// Number of multicellular organisms.
    pub multicell_count: usize,
    /// Number of specialised cells.
    pub specialized_count: usize,
    /// Number of symbol-active particles.
    pub symbol_count: usize,
    /// Number of tool-holding particles.
    pub tool_count: usize,
    /// Number of particles contributing to builds.
    pub build_count: usize,
    /// Number of culturally-active particles.
    pub culture_count: usize,
    /// Number of meta-cognitive organisms.
    pub metacog_count: usize,
}

impl Default for SimStats {
    fn default() -> Self {
        Self {
            tick: 0,
            particle_count: 0,
            bond_count: 0,
            organism_count: 0,
            colony_count: 0,
            total_energy: 0.0,
            max_generation: 0,
            era: SimEra::Primordial,
            day_phase: 0.0,
            ticks_per_second: 0.0,
            ms_per_tick: 0.0,
            deposit_count: 0,
            avg_fitness: 0.0,
            max_fitness: 0.0,
            total_predation: 0,
            total_reproduction: 0,
            multicell_count: 0,
            specialized_count: 0,
            symbol_count: 0,
            tool_count: 0,
            build_count: 0,
            culture_count: 0,
            metacog_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// History tracking
// ---------------------------------------------------------------------------

/// Rolling history buffers for key metrics (used for sparkline graphs in the UI).
/// Each buffer stores the last 200 data points (one per N ticks).
#[derive(Resource, Clone, Debug, Default)]
pub struct SimHistory {
    pub organisms: CircularBuffer<f32, 200>,
    pub bonds: CircularBuffer<f32, 200>,
    pub energy: CircularBuffer<f32, 200>,
    pub generation: CircularBuffer<f32, 200>,
    pub colonies: CircularBuffer<f32, 200>,
}

// ---------------------------------------------------------------------------
// Organism registry
// ---------------------------------------------------------------------------

/// Detailed information about a single organism.
#[derive(Debug, Clone)]
pub struct OrganismInfo {
    /// Set of entity handles for member particles.
    pub members: HashSet<u32>,
    /// Total energy of all member particles.
    pub energy: f32,
    /// Age of the organism in ticks.
    pub age: u32,
    /// Generation number (0 for spontaneously formed).
    pub generation: u32,
    /// Fitness score (composite of size, age, reproduction success, etc.).
    pub fitness: f32,
    /// Colony this organism belongs to (`-1` = none).
    pub colony_id: i32,
    /// Cooldown ticks remaining before next reproduction attempt.
    pub repro_cooldown: u32,
    /// Number of predation events this organism has committed.
    pub predation_count: u32,
    /// Number of energy deposits this organism has created.
    pub deposit_count: u32,
    /// Map from `CellRole::as_index()` to the set of entities with that role.
    pub cells: HashMap<u8, HashSet<u32>>,
    /// Degree of cell-role specialisation (0.0–1.0).
    pub specialization: f32,
    /// History of symbol codes emitted.
    pub symbol_history: Vec<u8>,
    /// Whether this organism qualifies as multicellular.
    pub is_multicellular: bool,
    /// Number of times a member has used a tool.
    pub tool_use_count: u32,
    /// Accumulated build score.
    pub build_score: f32,
    /// Cultural meme memory.
    pub cultural_memory: Vec<u16>,
    /// Meta-cognition depth.
    pub meta_cog_depth: f32,
    /// Parent organism ID (`-1` = spontaneously formed).
    pub parent_id: i32,
}

impl Default for OrganismInfo {
    fn default() -> Self {
        Self {
            members: HashSet::new(),
            energy: 0.0,
            age: 0,
            generation: 0,
            fitness: 0.0,
            colony_id: -1,
            repro_cooldown: 0,
            predation_count: 0,
            deposit_count: 0,
            cells: HashMap::new(),
            specialization: 0.0,
            symbol_history: Vec::new(),
            is_multicellular: false,
            tool_use_count: 0,
            build_score: 0.0,
            cultural_memory: Vec::new(),
            meta_cog_depth: 0.0,
            parent_id: -1,
        }
    }
}

/// Global registry of all organisms.
#[derive(Resource, Clone, Debug, Default)]
pub struct OrganismRegistry {
    pub organisms: HashMap<u32, OrganismInfo>,
    pub next_id: u32,
}

impl OrganismRegistry {
    /// Allocate the next organism ID and register an empty OrganismInfo.
    pub fn create(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.organisms.insert(id, OrganismInfo::default());
        id
    }

    /// Remove an organism by ID, returning its info if it existed.
    pub fn remove(&mut self, id: u32) -> Option<OrganismInfo> {
        self.organisms.remove(&id)
    }

    /// Get an organism by ID.
    pub fn get(&self, id: u32) -> Option<&OrganismInfo> {
        self.organisms.get(&id)
    }

    /// Get a mutable reference to an organism by ID.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut OrganismInfo> {
        self.organisms.get_mut(&id)
    }
}

// ---------------------------------------------------------------------------
// Colony registry
// ---------------------------------------------------------------------------

/// Information about a colony (group of cooperating organisms).
#[derive(Debug, Clone, Default)]
pub struct ColonyInfo {
    /// IDs of member organisms.
    pub organism_ids: HashSet<u32>,
    /// Bonds between member particles (inter-organism).
    pub bonds: Vec<(u32, u32)>,
    /// Age of the colony in ticks.
    pub age: u32,
}

/// Global registry of all colonies.
#[derive(Resource, Clone, Debug, Default)]
pub struct ColonyRegistry {
    pub colonies: HashMap<u32, ColonyInfo>,
    pub next_id: u32,
}

impl ColonyRegistry {
    pub fn create(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.colonies.insert(id, ColonyInfo::default());
        id
    }

    pub fn remove(&mut self, id: u32) -> Option<ColonyInfo> {
        self.colonies.remove(&id)
    }

    pub fn get(&self, id: u32) -> Option<&ColonyInfo> {
        self.colonies.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut ColonyInfo> {
        self.colonies.get_mut(&id)
    }
}

// ---------------------------------------------------------------------------
// Thermal vents
// ---------------------------------------------------------------------------

/// A single thermal vent that injects energy into nearby particles.
#[derive(Debug, Clone)]
pub struct Vent {
    pub position: Vec3,
    pub strength: f32,
    pub radius: f32,
}

/// List of all thermal vents in the world.
#[derive(Resource, Clone, Debug, Default)]
pub struct VentList(pub Vec<Vent>);

// ---------------------------------------------------------------------------
// Simulation counters
// ---------------------------------------------------------------------------

/// Global counters tracking cumulative events and the next particle ID.
#[derive(Resource, Clone, Debug)]
pub struct SimCounters {
    /// Current simulation tick (mirrored from SimStats for convenience).
    pub tick: u64,
    pub total_repro: u64,
    pub total_pred: u64,
    pub total_symbiogenesis: u64,
    pub total_sexual_repro: u64,
    pub next_particle_id: u32,
    /// Set of milestone keys already achieved (prevents re-announcing).
    pub milestones: HashSet<String>,
}

impl Default for SimCounters {
    fn default() -> Self {
        Self {
            tick: 0,
            total_repro: 0,
            total_pred: 0,
            total_symbiogenesis: 0,
            total_sexual_repro: 0,
            next_particle_id: 0,
            milestones: HashSet::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Event log
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    Bond,
    Organism,
    Reproduction,
    Death,
    Mutation,
    Environment,
    Predation,
    Colony,
    Deposit,
    Milestone,
    Combo,
    Metabolism,
    Gene,
    Wave,
    Specialize,
    Epigenetic,
    Symbol,
    Multicell,
    Tool,
    Build,
    Culture,
    Metacog,
    Symbiogenesis,
    Sexual,
    Immune,
}

/// The type / category of a simulation event (for filtering & styling).
impl EventType {
    /// Return a lowercase string tag for this event type (used by UI).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Bond => "bond",
            Self::Organism => "organism",
            Self::Reproduction => "reproduction",
            Self::Death => "death",
            Self::Mutation => "mutation",
            Self::Environment => "environment",
            Self::Predation => "predation",
            Self::Colony => "colony",
            Self::Deposit => "deposit",
            Self::Milestone => "milestone",
            Self::Combo => "combo",
            Self::Metabolism => "metabolism",
            Self::Gene => "gene",
            Self::Wave => "wave",
            Self::Specialize => "specialize",
            Self::Epigenetic => "epigenetic",
            Self::Symbol => "symbol",
            Self::Multicell => "multicell",
            Self::Tool => "tool",
            Self::Build => "build",
            Self::Culture => "culture",
            Self::Metacog => "metacog",
            Self::Symbiogenesis => "symbiogenesis",
            Self::Sexual => "sexual",
            Self::Immune => "immune",
        }
    }
}

impl From<&str> for EventType {
    fn from(s: &str) -> Self {
        match s {
            "bond" => EventType::Bond,
            "organism" => EventType::Organism,
            "reproduction" => EventType::Reproduction,
            "death" => EventType::Death,
            "mutation" => EventType::Mutation,
            "environment" => EventType::Environment,
            "predation" => EventType::Predation,
            "colony" => EventType::Colony,
            "deposit" => EventType::Deposit,
            "milestone" => EventType::Milestone,
            "combo" => EventType::Combo,
            "metabolism" => EventType::Metabolism,
            "gene" => EventType::Gene,
            "wave" => EventType::Wave,
            "specialize" => EventType::Specialize,
            "epigenetic" => EventType::Epigenetic,
            "symbol" => EventType::Symbol,
            "multicell" => EventType::Multicell,
            "tool" => EventType::Tool,
            "build" => EventType::Build,
            "culture" => EventType::Culture,
            "metacog" => EventType::Metacog,
            "symbiogenesis" => EventType::Symbiogenesis,
            "sexual" => EventType::Sexual,
            "immune" => EventType::Immune,
            _ => EventType::Environment,
        }
    }
}




/// A single event entry in the simulation log.
#[derive(Debug, Clone)]
pub struct EventEntry {
    pub tick: u64,
    pub text: String,
    pub event_type: EventType,
}

/// Bounded event log (ring-buffer semantics via `VecDeque`).
#[derive(Resource, Clone, Debug)]
pub struct EventLog {
    pub events: VecDeque<EventEntry>,
    pub max_size: usize,
}

impl Default for EventLog {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            max_size: 500,
        }
    }
}

impl EventLog {
    /// Push a new event, evicting the oldest if at capacity.
    pub fn push(&mut self, tick: u64, text: String, event_type: EventType) {
        if self.events.len() >= self.max_size {
            self.events.pop_front();
        }
        self.events.push_back(EventEntry {
            tick,
            text,
            event_type,
        });
    }

    /// Iterate over events from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &EventEntry> {
        self.events.iter()
    }


    /// Return the N most recent events (newest last).
    pub fn recent(&self, n: usize) -> Vec<&EventEntry> {
        let len = self.events.len();
        let skip = if len > n { len - n } else { 0 };
        self.events.iter().skip(skip).collect()
    }
}


// ---------------------------------------------------------------------------
// Phylogeny tree
// ---------------------------------------------------------------------------

/// A node in the phylogeny (evolutionary lineage) tree.
#[derive(Debug, Clone)]
pub struct PhyloNode {
    /// Parent organism ID (-1 = root / spontaneous).
    pub parent_id: i32,
    /// Tick at which this organism was created.
    pub tick: u64,
    /// Generation number.
    pub generation: u32,
    /// Size (number of member particles) at creation.
    pub size: usize,
}

/// The full phylogeny tree for all organisms ever created.
#[derive(Resource, Clone, Debug, Default)]
pub struct PhylogenyTree {
    pub nodes: HashMap<u32, PhyloNode>,
}

impl PhylogenyTree {
    /// Record a new organism in the tree.
    pub fn add(&mut self, organism_id: u32, parent_id: i32, tick: u64, generation: u32, size: usize) {
        self.nodes.insert(organism_id, PhyloNode {
            parent_id,
            tick,
            generation,
            size,
        });
    }
}

// ---------------------------------------------------------------------------
// Combo state
// ---------------------------------------------------------------------------

/// Tracks combo interactions — unique bond-pattern signatures and cached
/// per-particle combo bonuses.
#[derive(Resource, Clone, Debug, Default)]
pub struct ComboState {
    /// Set of unique combo signatures seen.
    pub signatures: HashSet<u32>,
    /// Cached combo bonus values per entity: `[bonus, factor1, factor2, factor3]`.
    pub cache: HashMap<u32, [f32; 4]>,
}

// ---------------------------------------------------------------------------
// Miscellaneous trackers / resources
// ---------------------------------------------------------------------------

/// Contact tracker for symbiogenesis: maps `(org_id_a, org_id_b)` →
/// number of ticks in sustained contact.
#[derive(Resource, Clone, Debug, Default)]
pub struct ContactTracker(pub HashMap<(u32, u32), u32>);

/// Maps organism ID → immune signature.
#[derive(Resource, Clone, Debug, Default)]
pub struct OrgSignatures(pub HashMap<u32, u32>);

/// Maps organism ID → list of active gene indices.
#[derive(Resource, Clone, Debug, Default)]
pub struct GeneMap(pub HashMap<u32, Vec<usize>>);

/// Set of currently active symbol codes across the simulation.
#[derive(Resource, Clone, Debug, Default)]
pub struct ActiveSymbolCodes(pub HashSet<u8>);

/// Set of currently active cultural meme IDs.
#[derive(Resource, Clone, Debug, Default)]
pub struct ActiveMemes(pub HashSet<u16>);

/// Maps discretised grid position `(x, y, z)` → build value / hit count.
#[derive(Resource, Clone, Debug, Default)]
pub struct BuildSites(pub HashMap<(i32, i32, i32), u32>);

/// Total number of completed build structures.
#[derive(Resource, Clone, Debug, Default)]
pub struct BuildStructureCount(pub u32);

/// Number of organisms exhibiting meta-cognition.
#[derive(Resource, Clone, Debug, Default)]
pub struct MetaCogOrgCount(pub u32);

/// Number of tool-grab events.
#[derive(Resource, Clone, Debug, Default)]
pub struct ToolGrabCount(pub u32);

/// Current metabolite flow rate (aggregate).
#[derive(Resource, Clone, Debug, Default)]
pub struct MetaboliteFlowRate(pub f32);

/// Number of active gene expressions across all organisms.
#[derive(Resource, Clone, Debug, Default)]
pub struct ActiveGeneCount(pub u32);

/// Number of cultural events this tick / recent window.
#[derive(Resource, Clone, Debug, Default)]
pub struct CulturalEventCount(pub u32);

/// Day/night cycle state.
#[derive(Resource, Clone, Debug)]
pub struct DayNightState {
    /// Whether it is currently daytime (phase < 0.5).
    pub is_day: bool,
    /// Current phase in the cycle (0.0–1.0).
    pub phase: f32,
    /// Current effective solar strength after day/night modulation.
    pub solar_now: f32,
}

impl Default for DayNightState {
    fn default() -> Self {
        Self {
            phase: 0.0,
            solar_now: 0.15,
            is_day: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar fields — holds all continuous field data for the simulation
// ---------------------------------------------------------------------------

/// All scalar fields used by the simulation: nutrients, pheromones,
/// metabolites, waves, and symbol channels.
///
/// These fields live on 3D grids (toroidal wrapping) and are sampled
/// by particles to influence their behaviour. They are updated each tick
/// with injection, diffusion, and decay.
#[derive(Resource)]
pub struct SimFields {
    /// Nutrient concentration field (grid resolution: 20³).
    /// Injected by thermal vents and deposits, consumed by organisms.
    pub nutrient: crate::util::ScalarField,

    /// Attractive pheromone field (grid resolution: 14³).
    /// Emitted by Catalyst particles in organisms to attract others.
    pub phero_attr: crate::util::ScalarField,

    /// Alarm pheromone field (grid resolution: 14³).
    /// Emitted on predation / bond-breaking events to warn nearby organisms.
    pub phero_alarm: crate::util::ScalarField,

    /// Trail pheromone field (grid resolution: 14³).
    /// Left behind by moving organisms for path-finding.
    pub phero_trail: crate::util::ScalarField,

    /// Wave amplitude field (grid resolution: 14³).
    /// Propagated wave energy used for communication.
    pub wave_amp: crate::util::ScalarField,

    /// Previous wave amplitude (for wave propagation computation).
    pub wave_prev: crate::util::ScalarField,

    /// Metabolite channel A (grid resolution: 14³).
    pub meta_a: crate::util::ScalarField,

    /// Metabolite channel B (grid resolution: 14³).
    pub meta_b: crate::util::ScalarField,

    /// Metabolite channel C (grid resolution: 14³).
    pub meta_c: crate::util::ScalarField,

    /// Symbol broadcast channels (8 channels, grid resolution: 14³ each).
    /// Used by symbol-active organisms for cultural communication.
    pub symbol: [crate::util::ScalarField; 8],
}

impl Default for SimFields {
    fn default() -> Self {
        Self {
            nutrient: crate::util::ScalarField::new(20),
            phero_attr: crate::util::ScalarField::new(14),
            phero_alarm: crate::util::ScalarField::new(14),
            phero_trail: crate::util::ScalarField::new(14),
            wave_amp: crate::util::ScalarField::new(14),
            wave_prev: crate::util::ScalarField::new(14),
            meta_a: crate::util::ScalarField::new(14),
            meta_b: crate::util::ScalarField::new(14),
            meta_c: crate::util::ScalarField::new(14),
            symbol: std::array::from_fn(|_| crate::util::ScalarField::new(14)),
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin helper — inserts all resources with defaults
// ---------------------------------------------------------------------------

/// Insert all simulation resources with their default values into the Bevy app.
///
/// Call this from your plugin's `build()` method:
/// ```ignore
/// app.add_plugins(SimResourcesPlugin);
/// ```
pub struct SimResourcesPlugin;

impl Plugin for SimResourcesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimMatrices>()
            .init_resource::<SimStats>()
            .init_resource::<SimHistory>()
            .init_resource::<OrganismRegistry>()
            .init_resource::<ColonyRegistry>()
            .init_resource::<VentList>()
            .init_resource::<SimCounters>()
            .init_resource::<EventLog>()
            .init_resource::<PhylogenyTree>()
            .init_resource::<ComboState>()
            .init_resource::<ContactTracker>()
            .init_resource::<OrgSignatures>()
            .init_resource::<GeneMap>()
            .init_resource::<ActiveSymbolCodes>()
            .init_resource::<ActiveMemes>()
            .init_resource::<BuildSites>()
            .init_resource::<BuildStructureCount>()
            .init_resource::<MetaCogOrgCount>()
            .init_resource::<ToolGrabCount>()
            .init_resource::<MetaboliteFlowRate>()
            .init_resource::<ActiveGeneCount>()
            .init_resource::<CulturalEventCount>()
            .init_resource::<DayNightState>();
    }
}
