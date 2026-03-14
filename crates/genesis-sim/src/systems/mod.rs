pub mod behavior;
pub mod bonds;
pub mod chemistry;
pub mod diffusion;
pub mod environment;
pub mod forces;
pub mod grid;
pub mod groups;
pub mod integrate;
pub mod metabolism;
pub mod metrics;
pub mod predation;
pub mod reproduction;
pub mod sensing;
pub mod signaling;

use bevy::prelude::*;

pub struct GenesisSystemsPlugin;

impl Plugin for GenesisSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                chemistry::chemistry_system,
                diffusion::diffusion_system,
                environment::environment_system,
                sensing::sensing_system,
                behavior::behavior_system,
                signaling::signaling_system,
                forces::forces_system,
                bonds::bonds_system,
                integrate::integrate_system,
                grid::wrap_system,
                predation::predation_system,
                metabolism::metabolism_system,
                reproduction::reproduction_system,
                groups::groups_system,
                metrics::metrics_system,
            )
                .chain(),
        );
    }
}
