//! Exploration module: ASAL-inspired open-ended search
//! 
//! Provides Novelty Search and Illumination Map for the parameter space.

use genesis_core::chemistry::NUM_CHEMICALS;
use genesis_core::genome::ComposableGenome;

/// A behavioral descriptor for a simulation run
#[derive(Clone, Debug)]
pub struct BehaviorDescriptor {
    /// Average chemical profile of the population
    pub avg_chem: [f32; NUM_CHEMICALS],
    /// Population size at end
    pub population: usize,
    /// Number of groups formed
    pub num_groups: usize,
    /// Average assembly index
    pub avg_assembly_index: f32,
    /// Maximum group size
    pub max_group_size: usize,
}

impl BehaviorDescriptor {
    pub fn distance(&self, other: &BehaviorDescriptor) -> f32 {
        let mut d = 0.0f32;
        for k in 0..NUM_CHEMICALS {
            let diff = self.avg_chem[k] - other.avg_chem[k];
            d += diff * diff;
        }
        let pop_diff = (self.population as f32 - other.population as f32) / 1000.0;
        d += pop_diff * pop_diff;
        let grp_diff = (self.num_groups as f32 - other.num_groups as f32) / 100.0;
        d += grp_diff * grp_diff;
        let ai_diff = self.avg_assembly_index - other.avg_assembly_index;
        d += ai_diff * ai_diff;
        d.sqrt()
    }
}

/// Novelty archive for novelty search
pub struct NoveltyArchive {
    pub archive: Vec<BehaviorDescriptor>,
    pub k_nearest: usize,
    pub threshold: f32,
}

impl NoveltyArchive {
    pub fn new(k_nearest: usize, threshold: f32) -> Self {
        Self {
            archive: Vec::new(),
            k_nearest,
            threshold,
        }
    }

    /// Compute novelty score for a descriptor
    pub fn novelty_score(&self, desc: &BehaviorDescriptor) -> f32 {
        if self.archive.is_empty() { return f32::MAX; }
        let mut distances: Vec<f32> = self.archive.iter()
            .map(|a| desc.distance(a))
            .collect();
        distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let k = self.k_nearest.min(distances.len());
        distances[..k].iter().sum::<f32>() / k as f32
    }

    /// Try to add a descriptor to the archive
    pub fn try_add(&mut self, desc: BehaviorDescriptor) -> bool {
        let score = self.novelty_score(&desc);
        if score > self.threshold {
            self.archive.push(desc);
            true
        } else {
            false
        }
    }
}

/// Illumination map: MAP-Elites style grid
pub struct IlluminationMap {
    pub grid_size: usize,
    pub cells: Vec<Option<(BehaviorDescriptor, ComposableGenome, f32)>>, // descriptor, genome, fitness
}

impl IlluminationMap {
    pub fn new(grid_size: usize) -> Self {
        let total = grid_size * grid_size;
        Self {
            grid_size,
            cells: vec![None; total],
        }
    }

    /// Map a descriptor to grid coordinates
    pub fn to_coords(&self, desc: &BehaviorDescriptor) -> (usize, usize) {
        // Use population and avg_assembly_index as the two axes
        let x = ((desc.population as f32 / 2000.0) * self.grid_size as f32) as usize;
        let y = (desc.avg_assembly_index * self.grid_size as f32) as usize;
        (x.min(self.grid_size - 1), y.min(self.grid_size - 1))
    }

    /// Try to place a solution in the map
    pub fn try_place(&mut self, desc: BehaviorDescriptor, genome: ComposableGenome, fitness: f32) -> bool {
        let (x, y) = self.to_coords(&desc);
        let idx = y * self.grid_size + x;
        match &self.cells[idx] {
            Some((_, _, existing_fitness)) if *existing_fitness >= fitness => false,
            _ => {
                self.cells[idx] = Some((desc, genome, fitness));
                true
            }
        }
    }

    /// Count filled cells
    pub fn coverage(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }
}
