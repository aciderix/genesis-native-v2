use bevy::prelude::*;
use genesis_core::chemistry::NUM_CHEMICALS;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Resource)]
pub struct SimConfig {
    // World
    pub world_width: f32,
    pub world_height: f32,
    pub initial_population: usize,
    pub max_particles: usize,

    // Physics
    pub dt: f32,
    pub friction: f32,
    pub interaction_radius: f32,
    pub bond_max_distance: f32,

    // Chemistry
    pub diffusion_rate: f32,
    pub reaction_rate_scale: f32,
    pub env_diffusion: f32,
    pub env_decay: f32,
    pub absorption_rate: f32,
    pub excretion_threshold: f32,

    // Forces (chemistry-driven)
    pub force_scale: f32,
    pub repulsion_strength: f32,
    pub bond_spring: f32,

    // Reproduction
    pub reproduction_energy_threshold: f32,
    pub reproduction_cost: f32,
    pub mutation_rate: f32,
    pub min_reproduction_age: u64,

    // Energy
    pub base_metabolism: f32,
    pub energy_from_reactions: f32,
    pub death_energy_threshold: f32,
    pub max_energy: f32,

    // Groups
    pub group_min_size: usize,

    // Environment
    pub num_env_fields: usize,
    pub env_source_strength: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            world_width: 800.0,
            world_height: 600.0,
            initial_population: 200,
            max_particles: 2000,
            dt: 0.016,
            friction: 0.97,
            interaction_radius: 50.0,
            bond_max_distance: 30.0,
            diffusion_rate: 0.05,
            reaction_rate_scale: 1.0,
            env_diffusion: 0.03,
            env_decay: 0.001,
            absorption_rate: 0.02,
            excretion_threshold: 0.8,
            force_scale: 1.0,
            repulsion_strength: 20.0,
            bond_spring: 0.5,
            reproduction_energy_threshold: 2.0,
            reproduction_cost: 1.2,
            mutation_rate: 0.05,
            min_reproduction_age: 100,
            base_metabolism: 0.002,
            energy_from_reactions: 1.0,
            death_energy_threshold: 0.0,
            max_energy: 5.0,
            group_min_size: 2,
            num_env_fields: NUM_CHEMICALS,
            env_source_strength: 0.01,
        }
    }
}
