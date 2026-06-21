use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use orion_voxels::{OrionVoxelGrid, VoxelConfig};
use ndshape::{RuntimeShape, Shape};

#[derive(Component)]
pub struct OrionMeshGenerated;

pub fn extract_surface_nets(
    grid: &OrionVoxelGrid,
    config: &VoxelConfig
) -> (Vec<Vec3>, Vec<u32>) {
    let size = config.array_size;
    let cell_count = size + 1;

    let cell_shape = RuntimeShape::<u32, 3>::new([cell_count, cell_count, cell_count]);
    let mut vertex_indices = vec![-1i32; cell_shape.size() as usize];

    // Step 1: Find voxels containing the surface and calculate their smooth vertices
    let vertices = generate_vertices(grid, config, &cell_shape, &mut vertex_indices);

    // Step 2: Connect those vertices into quads across surface-crossing edges
    let indices = generate_indices(grid, size, &cell_shape, &vertex_indices);

    (vertices, indices)
}

// --- STEP 1: VERTEX GENERATION SUB-PIPELINE ---

fn generate_vertices(
    grid: &OrionVoxelGrid,
    config: &VoxelConfig,
    cell_shape: &RuntimeShape<u32, 3>,
    vertex_indices: &mut [i32],
) -> Vec<Vec3> {
    let mut vertices = Vec::new();
    let size = config.array_size;
    let voxel_scale = config.grid_size / size as f32;

    for x in 0..=size {
        for y in 0..=size {
            for z in 0..=size {
                // FIXED: Pass voxel_scale so calculation loop computes uniform relative shifts
                if let Some(average_local_pos) = calculate_smooth_vertex(grid, x, y, z, voxel_scale) {
                    let local_position = average_local_pos * voxel_scale;

                    let cell_idx = cell_shape.linearize([x, y, z]) as usize;
                    vertex_indices[cell_idx] = vertices.len() as i32;
                    vertices.push(local_position);
                }
            }
        }
    }
    vertices
}

fn calculate_smooth_vertex(grid: &OrionVoxelGrid, x: u32, y: u32, z: u32, _voxel_scale: f32) -> Option<Vec3> {
    let mut corners_inside = 0;
    let mut corner_densities = [[[0.0; 2]; 2]; 2];

    for dx in 0..2 {
        for dy in 0..2 {
            for dz in 0..2 {
                let corner_pos = UVec3::new(x + dx, y + dy, z + dz);
                let density = grid.get_density(corner_pos);
                corner_densities[dx as usize][dy as usize][dz as usize] = density;

                if density <= 0.0 {
                    corners_inside += 1;
                }
            }
        }
    }

    if corners_inside == 0 || corners_inside == 8 {
        return None;
    }

    let local_cell_origin = Vec3::new(x as f32, y as f32, z as f32);
    let mut sum_positions = Vec3::ZERO;
    let mut edge_crossings = 0;

    // FIXED: The interpolation 't' parameter tells us exactly how far along an edge the
    // crossing occurs. The calculation needs to be purely relative to the local_cell_origin
    // to properly map local vertex data offsets.

    // 1. X-aligned edges
    for y_off in 0..2 {
        for z_off in 0..2 {
            let d0 = corner_densities[0][y_off][z_off];
            let d1 = corner_densities[1][y_off][z_off];
            if (d0 <= 0.0) != (d1 <= 0.0) {
                let t = d0 / (d0 - d1);
                sum_positions += local_cell_origin + Vec3::new(t, y_off as f32, z_off as f32);
                edge_crossings += 1;
            }
        }
    }
    // 2. Y-aligned edges
    for x_off in 0..2 {
        for z_off in 0..2 {
            let d0 = corner_densities[x_off][0][z_off];
            let d1 = corner_densities[x_off][1][z_off];
            if (d0 <= 0.0) != (d1 <= 0.0) {
                let t = d0 / (d0 - d1);
                sum_positions += local_cell_origin + Vec3::new(x_off as f32, t, z_off as f32);
                edge_crossings += 1;
            }
        }
    }
    // 3. Z-aligned edges
    for x_off in 0..2 {
        for y_off in 0..2 {
            let d0 = corner_densities[x_off][y_off][0];
            let d1 = corner_densities[x_off][y_off][1];
            if (d0 <= 0.0) != (d1 <= 0.0) {
                let t = d0 / (d0 - d1);
                sum_positions += local_cell_origin + Vec3::new(x_off as f32, y_off as f32, t);
                edge_crossings += 1;
            }
        }
    }

    let average_local_pos = if edge_crossings > 0 {
        sum_positions / edge_crossings as f32
    } else {
        local_cell_origin + Vec3::splat(0.5)
    };

    Some(average_local_pos)
}

// --- STEP 2: INDEX GENERATION SUB-PIPELINE ---

fn generate_indices(
    grid: &OrionVoxelGrid,
    size: u32,
    cell_shape: &RuntimeShape<u32, 3>,
    vertex_indices: &[i32],
) -> Vec<u32> {
    let mut indices = Vec::new();

    for x in 0..=size {
        for y in 0..=size {
            for z in 0..=size {
                let local_pos = UVec3::new(x, y, z);
                let current_density = grid.get_density(local_pos);

                let axes = [
                    (UVec3::X, 0),
                    (UVec3::Y, 1),
                    (UVec3::Z, 2),
                ];

                for (axis, axis_idx) in axes {
                    let neighbor_pos = local_pos + axis;
                    let neighbor_density = grid.get_density(neighbor_pos);

                    if (current_density <= 0.0) != (neighbor_density <= 0.0) {
                        collect_quad_indices(
                            x, y, z,
                            axis_idx,
                            current_density,
                            cell_shape,
                            vertex_indices,
                            &mut indices
                        );
                    }
                }
            }
        }
    }
    indices
}

fn collect_quad_indices(
    x: u32,
    y: u32,
    z: u32,
    axis_idx: usize,
    current_density: f32,
    cell_shape: &RuntimeShape<u32, 3>,
    vertex_indices: &[i32],
    indices: &mut Vec<u32>,
) {
    let cx = x as i32;
    let cy = y as i32;
    let cz = z as i32;

    let cells = match axis_idx {
        0 => [ // X-axis edge
            (cx, cy, cz),
            (cx, cy, cz - 1),
            (cx, cy - 1, cz - 1),
            (cx, cy - 1, cz),
        ],
        1 => [ // Y-axis edge
            (cx, cy, cz),
            (cx - 1, cy, cz),
            (cx - 1, cy, cz - 1),
            (cx, cy, cz - 1),
        ],
        _ => [ // Z-axis edge
            (cx, cy, cz),
            (cx, cy - 1, cz),
            (cx - 1, cy - 1, cz),
            (cx - 1, cy, cz),
        ],
    };

    let mut quad_v_indices = [0u32; 4];
    let cell_limits = cell_shape.as_array();

    for i in 0..4 {
        let (ccx, ccy, ccz) = cells[i];

        if ccx < 0 || ccx >= cell_limits[0] as i32 ||
            ccy < 0 || ccy >= cell_limits[1] as i32 ||
            ccz < 0 || ccz >= cell_limits[2] as i32 {
            return;
        }

        let cell_idx = cell_shape.linearize([ccx as u32, ccy as u32, ccz as u32]) as usize;
        let v_idx = vertex_indices[cell_idx];
        if v_idx < 0 {
            return;
        }
        quad_v_indices[i] = v_idx as u32;
    }

    let [i0, i1, i2, i3] = quad_v_indices;

    if current_density <= 0.0 {
        indices.extend_from_slice(&[i0, i2, i1]);
        indices.extend_from_slice(&[i0, i3, i2]);
    } else {
        indices.extend_from_slice(&[i0, i1, i2]);
        indices.extend_from_slice(&[i0, i2, i3]);
    }
}

fn process_static_voxel_grids(
    mut commands: Commands,
    config: Res<VoxelConfig>,
    query: Query<(Entity, &OrionVoxelGrid), Without<OrionMeshGenerated>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, voxel_grid) in &query {
        let (positions, indices) = extract_surface_nets(voxel_grid, &config);

        if !positions.is_empty() && !indices.is_empty() {
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_indices(Indices::U32(indices));

            mesh.duplicate_vertices();
            mesh.compute_flat_normals();

            let chunk_world_pos = voxel_grid.coord.as_vec3() * config.grid_size;

            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    ..default()
                })),
                Transform::from_translation(chunk_world_pos),
                Visibility::default(),
            ));
        }

        commands.entity(entity).insert(OrionMeshGenerated);
    }
}

pub struct OrionMeshPlugin;

impl Plugin for OrionMeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, process_static_voxel_grids);
    }
}