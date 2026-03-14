use bevy::prelude::*;
use genesis_core::chemistry::NUM_CHEMICALS;
use genesis_core::metrics::MetricsSnapshot;

/// Simulation tick counter
#[derive(Resource, Default)]
pub struct SimTick(pub u64);

/// Simulation pause state
#[derive(Resource)]
pub struct SimPaused(pub bool);

impl Default for SimPaused {
    fn default() -> Self {
        Self(false)
    }
}

/// Group registry: tracks connected components of bonded particles
#[derive(Resource, Default)]
pub struct GroupRegistry {
    pub groups: Vec<Group>,
}

pub struct Group {
    pub id: u32,
    pub member_count: usize,
    pub center_x: f32,
    pub center_y: f32,
    pub avg_chem: [f32; NUM_CHEMICALS],
    pub total_energy: f32,
}

/// Environment chemical fields (grid-based)
#[derive(Resource)]
pub struct EnvironmentFields {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub fields: Vec<Vec<f32>>,
}

impl Default for EnvironmentFields {
    fn default() -> Self {
        let width = 80;
        let height = 60;
        let size = width * height;
        Self {
            width,
            height,
            cell_size: 10.0,
            fields: vec![vec![0.0; size]; NUM_CHEMICALS],
        }
    }
}

impl EnvironmentFields {
    pub fn get(&self, field: usize, gx: usize, gy: usize) -> f32 {
        if field < NUM_CHEMICALS && gx < self.width && gy < self.height {
            self.fields[field][gy * self.width + gx]
        } else {
            0.0
        }
    }

    pub fn set(&mut self, field: usize, gx: usize, gy: usize, val: f32) {
        if field < NUM_CHEMICALS && gx < self.width && gy < self.height {
            self.fields[field][gy * self.width + gx] = val.clamp(0.0, 1.0);
        }
    }

    pub fn add(&mut self, field: usize, gx: usize, gy: usize, val: f32) {
        if field < NUM_CHEMICALS && gx < self.width && gy < self.height {
            let idx = gy * self.width + gx;
            self.fields[field][idx] = (self.fields[field][idx] + val).clamp(0.0, 1.0);
        }
    }

    pub fn world_to_grid(&self, wx: f32, wy: f32) -> (usize, usize) {
        let gx = ((wx / self.cell_size) as usize).min(self.width.saturating_sub(1));
        let gy = ((wy / self.cell_size) as usize).min(self.height.saturating_sub(1));
        (gx, gy)
    }

    /// Diffuse all fields (blur step)
    pub fn diffuse(&mut self, rate: f32) {
        for field in 0..NUM_CHEMICALS {
            let old = self.fields[field].clone();
            for y in 1..self.height - 1 {
                for x in 1..self.width - 1 {
                    let idx = y * self.width + x;
                    let avg = (old[idx - 1] + old[idx + 1] + old[idx - self.width]
                        + old[idx + self.width])
                        * 0.25;
                    self.fields[field][idx] += (avg - old[idx]) * rate;
                }
            }
        }
    }

    /// Decay all fields
    pub fn decay(&mut self, rate: f32) {
        for field in &mut self.fields {
            for v in field.iter_mut() {
                *v *= 1.0 - rate;
            }
        }
    }
}

/// Metrics history for UI display
#[derive(Resource, Default)]
pub struct MetricsHistory {
    pub snapshots: Vec<MetricsSnapshot>,
    pub known_reactions: std::collections::HashSet<u64>,
}

/// RNG resource using a simple xorshift
#[derive(Resource)]
pub struct SimRng {
    state: u64,
}

impl Default for SimRng {
    fn default() -> Self {
        Self {
            state: 12345678901234567,
        }
    }
}

impl SimRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Returns a float in [0.0, 1.0)
    pub fn next_f32(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as u32 as f32) / (u32::MAX as f32)
    }

    /// Returns a float in [min, max)
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    /// Returns an integer in [0, n)
    pub fn next_usize(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_f32() * n as f32) as usize % n
    }
}

// ── Phylogenetic tree ───────────────────────────────────────────────────────

/// A node in the phylogenetic tree.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PhyloNode {
    pub parent_id: i32,
    pub tick: u64,
    pub generation: u32,
    pub genome_hash: u64,
    pub size: usize,
}

/// Full phylogenetic tree tracking all lineages.
#[derive(Resource, Default, Clone, Debug)]
pub struct PhylogenyTree {
    pub nodes: std::collections::HashMap<u32, PhyloNode>,
}

impl PhylogenyTree {
    /// Register a birth event.
    pub fn register_birth(&mut self, id: u32, parent_id: i32, tick: u64, generation: u32, genome_hash: u64) {
        self.nodes.insert(id, PhyloNode {
            parent_id,
            tick,
            generation,
            genome_hash,
            size: 1,
        });
    }

    /// Count total nodes.
    pub fn size(&self) -> usize {
        self.nodes.len()
    }

    /// Compute tree depth (longest path to root).
    pub fn max_depth(&self) -> u32 {
        self.nodes.values().map(|n| n.generation).max().unwrap_or(0)
    }

    /// Count unique lineages (nodes with no children that are roots).
    pub fn num_leaves(&self) -> usize {
        let parents: std::collections::HashSet<i32> = self.nodes.values()
            .map(|n| n.parent_id)
            .collect();
        // Nodes whose id is not a parent of anyone
        self.nodes.keys()
            .filter(|&&id| !parents.contains(&(id as i32)))
            .count()
    }

    /// Prune old nodes to keep memory bounded (keep last N generations).
    pub fn prune(&mut self, max_generation_depth: u32) {
        if self.nodes.is_empty() { return; }
        let max_gen = self.max_depth();
        if max_gen <= max_generation_depth { return; }
        let cutoff = max_gen - max_generation_depth;
        self.nodes.retain(|_, n| n.generation >= cutoff);
    }
}

/// Simulation counters for save/load compatibility.
#[derive(Resource, Default, Clone, Debug)]
pub struct SimCounters {
    pub total_repro: u64,
    pub total_pred: u64,
    pub total_sexual_repro: u64,
}
