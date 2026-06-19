use bevy::prelude::*;

pub struct HelloTriangle;

impl HelloTriangle {
    fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
        commands.spawn(Camera2d);

        let triangle = Triangle2d::new(Vec2::new(0f32, 100f32), Vec2::new(100f32, 0f32), Vec2::new(-100f32, 0f32));

        let mut mesh = Mesh::from(triangle);

        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![LinearRgba::RED.to_f32_array(), LinearRgba::GREEN.to_f32_array(), LinearRgba::BLUE.to_f32_array()]);

        let mesh_handle = meshes.add(mesh);

        commands.spawn((
            Mesh2d(mesh_handle),
            MeshMaterial2d(materials.add(ColorMaterial::default())),
        ));
    }
}

impl Plugin for HelloTriangle {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, Self::setup);
    }
}