use bevy::prelude::*;
use genesis_core::cell_role::CellRole;
use genesis_core::chemistry::NUM_CHEMICALS;
use genesis_core::genome::ComposableGenome;

/// Marker for particles in the simulation
#[derive(Component, Clone, Debug)]
pub struct Particle;

/// Chemical state of a particle: 8 concentrations in [0,1]
#[derive(Component, Clone, Debug)]
pub struct ChemState {
    pub chem: [f32; NUM_CHEMICALS],
}

impl Default for ChemState {
    fn default() -> Self {
        Self {
            chem: [0.0; NUM_CHEMICALS],
        }
    }
}

/// Genome component: composable list of chemical reactions
#[derive(Component, Clone, Debug)]
pub struct Genome(pub ComposableGenome);

/// Velocity of a particle
#[derive(Component, Clone, Debug, Default)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

/// Energy level of a particle
#[derive(Component, Clone, Debug)]
pub struct Energy(pub f32);

impl Default for Energy {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Bond between two particles
#[derive(Component, Clone, Debug)]
pub struct Bond {
    pub partner: Entity,
    pub strength: f32,
}

/// Group membership (replaces old organism/colony)
#[derive(Component, Clone, Debug)]
pub struct GroupId(pub u32);

/// Parent lineage tracking
#[derive(Component, Clone, Debug)]
pub struct Lineage {
    pub parent_id: Option<u32>,
    pub generation: u32,
    pub birth_tick: u64,
}

/// Age of a particle in ticks
#[derive(Component, Clone, Debug, Default)]
pub struct Age(pub u64);

/// Unique ID for each particle (for lineage tracking)
#[derive(Component, Clone, Debug)]
pub struct ParticleId(pub u32);

/// Particle type enum for save/load compatibility
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum ParticleType {
    Alpha,
}

impl ParticleType {
    pub fn as_index(&self) -> usize {
        match self {
            ParticleType::Alpha => 0,
        }
    }

    pub fn try_from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(ParticleType::Alpha),
            _ => None,
        }
    }
}
