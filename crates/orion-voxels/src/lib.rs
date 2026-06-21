use bevy::prelude::*;
use ndshape::{RuntimeShape, Shape};
use orion_sdf::SDF;

#[derive(Component)]
pub struct OrionVoxelGrid {
    pub coord: IVec3,
    pub shape: RuntimeShape<u32, 3>,
    pub data: Vec<f32>,
}

impl OrionVoxelGrid {
    pub fn new(voxel_array_size: u32, voxel_grid_size: f32, coord: IVec3, sdf: &impl SDF) -> OrionVoxelGrid {
        let mut data = Vec::with_capacity((voxel_array_size * voxel_array_size * voxel_array_size) as usize);

        let voxel_grid_offset = coord.as_vec3() * voxel_grid_size;

        let voxel_scale = voxel_grid_size / voxel_array_size as f32;
        let half_voxel_offset = Vec3::splat(voxel_scale * 0.5);

        for x in 0..voxel_array_size {
            for y in 0..voxel_array_size {
                for z in 0..voxel_array_size {
                    let voxel_relative_position = UVec3::new(x, y, z);
                    let voxel_position = voxel_grid_offset + voxel_relative_position.as_vec3() * voxel_scale + half_voxel_offset;

                    data.push(sdf.evaluate(voxel_position));
                }
            }
        }

        OrionVoxelGrid {
            coord,
            shape: RuntimeShape::<u32, 3>::new([voxel_array_size, voxel_array_size, voxel_array_size]),
            data,
        }
    }

    pub fn get_density(&self, local_pos: UVec3) -> f32 {
        let linear_index = self.shape.linearize(local_pos.to_array());
        self.data[linear_index as usize]
    }
}

#[derive(Resource, Clone, Copy)]
pub struct VoxelConfig {
    pub array_size: u32,
    pub grid_size: f32,
}

pub struct OrionVoxelPlugin;

impl Plugin for OrionVoxelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VoxelConfig {
            array_size: 32,
            grid_size: 1.0,
        });
    }
}