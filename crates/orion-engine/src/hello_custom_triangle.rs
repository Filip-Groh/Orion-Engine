use bevy::{
    prelude::*,
    reflect::TypePath,
    shader::ShaderRef,
    render::render_resource::{AsBindGroup, ShaderType, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    pbr::{MaterialPipeline, MaterialPipelineKey},
};
use bevy::mesh::{MeshVertexBufferLayoutRef, VertexFormat};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct HelloTriangleMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Material for HelloTriangleMaterial {
    fn vertex_shader() -> ShaderRef { "shaders/generated/vertex.spv".into() }
    fn fragment_shader() -> ShaderRef { "shaders/generated/fragment.spv".into() }
}

pub struct HelloTrianglePlugin;

impl Plugin for HelloTrianglePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<HelloTriangleMaterial>::default())
            .add_systems(Startup, setup_triangle_scene);
    }
}

#[derive(Component)]
pub struct CustomTriangle;

fn setup_triangle_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HelloTriangleMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(HelloTriangleMaterial {
            color: LinearRgba::RED
        })),
        Transform::from_scale(Vec3::splat(2.0)),
        CustomTriangle,
        Visibility::Visible,
    ));
}