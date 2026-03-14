use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use genesis_core::chemistry::diffuse_chemicals;

/// Chemical diffusion between bonded particles.
pub fn diffusion_system(mut store: ResMut<ParticleStore>, config: Res<SimConfig>) {
    // Collect bond pairs to avoid double-borrow
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for i in 0..store.count {
        if !store.alive[i] {
            continue;
        }
        for &(j, _strength) in &store.bonds[i] {
            if j > i && j < store.count && store.alive[j] {
                pairs.push((i, j));
            }
        }
    }
    // Apply diffusion for each bonded pair
    for (i, j) in pairs {
        let mut chem_i = store.chem[i];
        let mut chem_j = store.chem[j];
        diffuse_chemicals(&mut chem_i, &mut chem_j, config.diffusion_rate);
        store.chem[i] = chem_i;
        store.chem[j] = chem_j;
    }
}
