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
