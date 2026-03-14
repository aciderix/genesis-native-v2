use bevy::prelude::*;
use crate::config::SimConfig;
use crate::particle_store::ParticleStore;
use crate::resources::GroupRegistry;
use genesis_core::cell_role::CellRole;
use genesis_core::chemistry::NUM_CHEMICALS;

/// Detect connected components of bonded particles (replaces organisms + colonies).
/// Also assigns cell differentiation roles (Interior/Border).
pub fn groups_system(
    mut store: ResMut<ParticleStore>,
    config: Res<SimConfig>,
    mut registry: ResMut<GroupRegistry>,
) {
    let count = store.count;
    // Union-Find for connected components
    let mut parent: Vec<usize> = (0..count).collect();

    fn find(parent: &mut [usize], x: usize) -> usize {
        let mut root = x;
        while parent[root] != root {
            root = parent[root];
        }
        // Path compression
        let mut cur = x;
        while parent[cur] != root {
            let next = parent[cur];
            parent[cur] = root;
            cur = next;
        }
        root
    }

    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    // Build components from bonds
    for i in 0..count {
        if !store.alive[i] {
            continue;
        }
        for &(j, _) in &store.bonds[i] {
            if j < count && store.alive[j] {
                union(&mut parent, i, j);
            }
        }
    }

    // Assign group IDs
    let mut group_map: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    let mut next_group_id = 0u32;
    for i in 0..count {
        if !store.alive[i] {
            store.group_ids[i] = -1;
            continue;
        }
        let root = find(&mut parent, i);
        let gid = *group_map.entry(root).or_insert_with(|| {
            let id = next_group_id;
            next_group_id += 1;
            id
        });
        store.group_ids[i] = gid as i32;
    }

    // Build group registry
    registry.groups.clear();
    let mut group_data: std::collections::HashMap<
        u32,
        (usize, f32, f32, [f32; NUM_CHEMICALS], f32),
    > = std::collections::HashMap::new();
    for i in 0..count {
        if !store.alive[i] || store.group_ids[i] < 0 {
            continue;
        }
        let gid = store.group_ids[i] as u32;
        let entry = group_data
            .entry(gid)
            .or_insert((0, 0.0, 0.0, [0.0; NUM_CHEMICALS], 0.0));
        entry.0 += 1;
        entry.1 += store.x[i];
        entry.2 += store.y[i];
        for k in 0..NUM_CHEMICALS {
            entry.3[k] += store.chem[i][k];
        }
        entry.4 += store.energy[i];
    }

    // Count group sizes for role assignment
    let mut group_sizes: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
    for i in 0..count {
        if store.alive[i] && store.group_ids[i] >= 0 {
            *group_sizes.entry(store.group_ids[i]).or_insert(0) += 1;
        }
    }

    // ── Cell Differentiation: assign roles ──────────────────────────────
    for i in 0..count {
        if !store.alive[i] {
            store.roles[i] = CellRole::Undifferentiated;
            continue;
        }

        let gid = store.group_ids[i];
        let gsize = group_sizes.get(&gid).copied().unwrap_or(1);

        if gsize < config.group_min_size {
            // Solo particles are undifferentiated
            store.roles[i] = CellRole::Undifferentiated;
        } else {
            // Determine role based on bond count relative to group
            let _bond_count = store.bonds[i].len();
            // Count how many bonds connect to same-group members
            let group_bond_count = store.bonds[i].iter()
                .filter(|&&(j, _)| j < count && store.alive[j] && store.group_ids[j] == gid)
                .count();

            if group_bond_count >= 3 {
                // Well-connected → interior cell
                store.roles[i] = CellRole::Interior;
            } else {
                // Fewer connections → border cell
                store.roles[i] = CellRole::Border;
            }
        }
    }

    // Build registry entries
    for (id, (member_count, cx, cy, mut avg_chem, total_energy)) in group_data {
        if member_count < config.group_min_size {
            continue;
        }
        let n = member_count as f32;
        for k in 0..NUM_CHEMICALS {
            avg_chem[k] /= n;
        }
        registry.groups.push(crate::resources::Group {
            id,
            member_count,
            center_x: cx / n,
            center_y: cy / n,
            avg_chem,
            total_energy,
        });
    }
}
