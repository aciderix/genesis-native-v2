//! Save / Load — JSON serialization for Genesis 2.0 simulation state.
//!
//! Serializes and deserializes the particle store, counters, and phylogeny.

use serde::{Deserialize, Serialize};

use crate::particle_store::ParticleStore;
use crate::resources::{PhylogenyTree, PhyloNode, SimCounters};
use genesis_core::cell_role::CellRole;
use genesis_core::chemistry::NUM_CHEMICALS;
use genesis_core::genome::ComposableGenome;

// ─────────────────────────────────────────────────────────────────────────────
// Serializable snapshot types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct SaveState {
    pub version: u32,
    pub tick: u64,
    pub total_repro: u64,
    pub total_pred: u64,
    pub total_sexual_repro: u64,
    pub particles: Vec<SaveParticle>,
    pub phylogeny: Vec<(u32, SavePhyloNode)>,
}

#[derive(Serialize, Deserialize)]
pub struct SaveParticle {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub energy: f32,
    pub chem: [f32; NUM_CHEMICALS],
    pub genome: ComposableGenome,
    pub age: u64,
    pub parent_id: i32,
    pub generation: u32,
    pub group_id: i32,
    pub bonds: Vec<(usize, f32)>,
    pub role: u8,
}

#[derive(Serialize, Deserialize)]
pub struct SavePhyloNode {
    pub parent_id: i32,
    pub tick: u64,
    pub generation: u32,
    pub genome_hash: u64,
    pub size: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Serialize
// ─────────────────────────────────────────────────────────────────────────────

pub fn serialize_state(
    store: &ParticleStore,
    counters: &SimCounters,
    phylogeny: &PhylogenyTree,
    tick: u64,
) -> String {
    let particles: Vec<SaveParticle> = (0..store.count)
        .filter(|&i| store.alive[i])
        .map(|i| SaveParticle {
            id: store.particle_ids[i],
            x: store.x[i],
            y: store.y[i],
            vx: store.vx[i],
            vy: store.vy[i],
            energy: store.energy[i],
            chem: store.chem[i],
            genome: store.genomes[i].clone(),
            age: store.ages[i],
            parent_id: store.parent_ids[i],
            generation: store.generations[i],
            group_id: store.group_ids[i],
            bonds: store.bonds[i].clone(),
            role: store.roles[i].as_index() as u8,
        })
        .collect();

    let phylo_entries: Vec<(u32, SavePhyloNode)> = phylogeny
        .nodes
        .iter()
        .map(|(&id, node)| {
            (id, SavePhyloNode {
                parent_id: node.parent_id,
                tick: node.tick,
                generation: node.generation,
                genome_hash: node.genome_hash,
                size: node.size,
            })
        })
        .collect();

    let state = SaveState {
        version: 7,
        tick,
        total_repro: counters.total_repro,
        total_pred: counters.total_pred,
        total_sexual_repro: counters.total_sexual_repro,
        particles,
        phylogeny: phylo_entries,
    };

    serde_json::to_string(&state).unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// Deserialize
// ─────────────────────────────────────────────────────────────────────────────

pub fn deserialize_state(
    json: &str,
    store: &mut ParticleStore,
    counters: &mut SimCounters,
    phylogeny: &mut PhylogenyTree,
) -> Result<u64, String> {
    let data: SaveState =
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;

    *store = ParticleStore::default();

    for p in &data.particles {
        let idx = store.add_particle(
            p.x, p.y, p.chem, p.genome.clone(), p.energy, p.parent_id, p.generation,
        );
        store.vx[idx] = p.vx;
        store.vy[idx] = p.vy;
        store.ages[idx] = p.age;
        store.group_ids[idx] = p.group_id;
        store.bonds[idx] = p.bonds.clone();
        store.roles[idx] = CellRole::from_index(p.role as usize);
    }

    counters.total_repro = data.total_repro;
    counters.total_pred = data.total_pred;
    counters.total_sexual_repro = data.total_sexual_repro;

    phylogeny.nodes.clear();
    for (id, node) in data.phylogeny {
        phylogeny.nodes.insert(id, PhyloNode {
            parent_id: node.parent_id,
            tick: node.tick,
            generation: node.generation,
            genome_hash: node.genome_hash,
            size: node.size,
        });
    }

    Ok(data.tick)
}
