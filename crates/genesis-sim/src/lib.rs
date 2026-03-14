pub mod components;
pub mod config;
pub mod particle_store;
pub mod resources;
pub mod systems;

use bevy::prelude::*;
use config::SimConfig;
use particle_store::ParticleStore;
use resources::*;

pub struct GenesisSimPlugin;

impl Plugin for GenesisSimPlugin {
    fn build(&self, app: &mut App) {
        let config = SimConfig::default();
        app.insert_resource(config)
            .insert_resource(ParticleStore::default())
            .insert_resource(SimTick::default())
            .insert_resource(SimPaused::default())
            .insert_resource(GroupRegistry::default())
            .insert_resource(EnvironmentFields::default())
            .insert_resource(MetricsHistory::default())
            .insert_resource(SimRng::default())
            .add_plugins(systems::GenesisSystemsPlugin)
            .add_systems(Startup, spawn_initial_particles);
    }
}

fn spawn_initial_particles(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut rng: ResMut<SimRng>,
) {
    for _ in 0..config.initial_population {
        let x = rng.range(0.0, config.world_width);
        let y = rng.range(0.0, config.world_height);
        let mut chem = [0.0f32; genesis_core::chemistry::NUM_CHEMICALS];
        for c in chem.iter_mut() {
            *c = rng.next_f32();
        }
        let mut rng_fn = || rng.next_f32();
        let genome = genesis_core::genome::ComposableGenome::random(&mut rng_fn);
        let energy = rng.range(1.0, 3.0);
        store.add_particle(x, y, chem, genome, energy, -1, 0);
    }
}
