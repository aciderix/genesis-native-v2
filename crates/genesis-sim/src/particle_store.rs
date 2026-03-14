use bevy::prelude::*;
use genesis_core::cell_role::CellRole;
use genesis_core::chemistry::NUM_CHEMICALS;
use genesis_core::genome::ComposableGenome;

/// Sensor data computed each tick for each particle.
#[derive(Clone, Debug, Default)]
pub struct SensorInput {
    /// Own chemical concentrations.
    pub local_chem: [f32; NUM_CHEMICALS],
    /// Environment chemicals at this particle's position.
    pub env_chem: [f32; NUM_CHEMICALS],
    /// Average chemistry of nearby particles.
    pub neighbor_avg_chem: [f32; NUM_CHEMICALS],
    /// Own energy level.
    pub energy: f32,
    /// Number of neighbors within interaction radius.
    pub neighbor_count: usize,
    /// Number of active bonds.
    pub bond_count: usize,
    /// Size of the group this particle belongs to.
    pub group_size: usize,
    /// Distance to nearest neighbor (or f32::MAX if none).
    pub nearest_distance: f32,
    /// Direction to nearest neighbor (unit vector).
    pub nearest_dir: [f32; 2],
    /// Gradient of signal field (chem channel 3) at position [gx, gy].
    pub signal_gradient: [f32; 2],
}

/// SoA (Structure of Arrays) particle store for efficient batch processing
#[derive(Resource, Clone, Debug)]
pub struct ParticleStore {
    pub count: usize,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    pub energy: Vec<f32>,
    pub chem: Vec<[f32; NUM_CHEMICALS]>,
    pub genomes: Vec<ComposableGenome>,
    pub ages: Vec<u64>,
    pub parent_ids: Vec<i32>,
    pub generations: Vec<u32>,
    pub group_ids: Vec<i32>,
    pub particle_ids: Vec<u32>,
    pub alive: Vec<bool>,
    pub bonds: Vec<Vec<(usize, f32)>>,
    /// Cell differentiation roles (assigned by groups_system).
    pub roles: Vec<CellRole>,
    /// Sensor inputs (populated by sensing_system each tick).
    pub sensors: Vec<SensorInput>,
    /// Signal emissions queued by behavior_system: Vec of (channel, amount).
    pub signal_emit: Vec<Vec<(usize, f32)>>,
    /// Behavior-driven velocity deltas (accumulated by behavior_system).
    pub behavior_vx: Vec<f32>,
    pub behavior_vy: Vec<f32>,
    next_id: u32,
}

impl Default for ParticleStore {
    fn default() -> Self {
        Self {
            count: 0,
            x: Vec::new(),
            y: Vec::new(),
            vx: Vec::new(),
            vy: Vec::new(),
            energy: Vec::new(),
            chem: Vec::new(),
            genomes: Vec::new(),
            ages: Vec::new(),
            parent_ids: Vec::new(),
            generations: Vec::new(),
            group_ids: Vec::new(),
            particle_ids: Vec::new(),
            alive: Vec::new(),
            bonds: Vec::new(),
            roles: Vec::new(),
            sensors: Vec::new(),
            signal_emit: Vec::new(),
            behavior_vx: Vec::new(),
            behavior_vy: Vec::new(),
            next_id: 0,
        }
    }
}

impl ParticleStore {
    pub fn add_particle(
        &mut self,
        x: f32,
        y: f32,
        chem: [f32; NUM_CHEMICALS],
        genome: ComposableGenome,
        energy: f32,
        parent_id: i32,
        generation: u32,
    ) -> usize {
        let idx = self.count;
        self.x.push(x);
        self.y.push(y);
        self.vx.push(0.0);
        self.vy.push(0.0);
        self.energy.push(energy);
        self.chem.push(chem);
        self.genomes.push(genome);
        self.ages.push(0);
        self.parent_ids.push(parent_id);
        self.generations.push(generation);
        self.group_ids.push(-1);
        self.particle_ids.push(self.next_id);
        self.alive.push(true);
        self.bonds.push(Vec::new());
        self.roles.push(CellRole::Undifferentiated);
        self.sensors.push(SensorInput::default());
        self.signal_emit.push(Vec::new());
        self.behavior_vx.push(0.0);
        self.behavior_vy.push(0.0);
        self.next_id += 1;
        self.count += 1;
        idx
    }

    pub fn kill(&mut self, idx: usize) {
        if idx < self.count {
            self.alive[idx] = false;
        }
    }

    pub fn compact(&mut self) {
        // Build old-index → new-index mapping for bond remapping
        let mut old_to_new: Vec<Option<usize>> = vec![None; self.count];
        let mut write = 0;
        for read in 0..self.count {
            if self.alive[read] {
                old_to_new[read] = Some(write);
                write += 1;
            }
        }
        let new_count = write;

        // Compact all arrays
        write = 0;
        for read in 0..self.count {
            if self.alive[read] {
                if write != read {
                    self.x[write] = self.x[read];
                    self.y[write] = self.y[read];
                    self.vx[write] = self.vx[read];
                    self.vy[write] = self.vy[read];
                    self.energy[write] = self.energy[read];
                    self.chem[write] = self.chem[read];
                    self.genomes[write] = self.genomes[read].clone();
                    self.ages[write] = self.ages[read];
                    self.parent_ids[write] = self.parent_ids[read];
                    self.generations[write] = self.generations[read];
                    self.group_ids[write] = self.group_ids[read];
                    self.particle_ids[write] = self.particle_ids[read];
                    self.alive[write] = true;
                    self.bonds[write] = self.bonds[read].clone();
                    self.roles[write] = self.roles[read];
                    self.sensors[write] = self.sensors[read].clone();
                    self.signal_emit[write] = self.signal_emit[read].clone();
                    self.behavior_vx[write] = self.behavior_vx[read];
                    self.behavior_vy[write] = self.behavior_vy[read];
                }
                write += 1;
            }
        }

        // Truncate all arrays
        self.count = new_count;
        self.x.truncate(new_count);
        self.y.truncate(new_count);
        self.vx.truncate(new_count);
        self.vy.truncate(new_count);
        self.energy.truncate(new_count);
        self.chem.truncate(new_count);
        self.genomes.truncate(new_count);
        self.ages.truncate(new_count);
        self.parent_ids.truncate(new_count);
        self.generations.truncate(new_count);
        self.group_ids.truncate(new_count);
        self.particle_ids.truncate(new_count);
        self.alive.truncate(new_count);
        self.bonds.truncate(new_count);
        self.roles.truncate(new_count);
        self.sensors.truncate(new_count);
        self.signal_emit.truncate(new_count);
        self.behavior_vx.truncate(new_count);
        self.behavior_vy.truncate(new_count);

        // Remap bond indices
        for i in 0..self.count {
            self.bonds[i] = self.bonds[i]
                .iter()
                .filter_map(|&(old_partner, strength)| {
                    if old_partner < old_to_new.len() {
                        old_to_new[old_partner].map(|new_idx| (new_idx, strength))
                    } else {
                        None
                    }
                })
                .collect();
        }
    }

    pub fn population(&self) -> usize {
        self.alive.iter().filter(|&&a| a).count()
    }
}
