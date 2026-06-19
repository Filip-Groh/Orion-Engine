use bevy::prelude::*;

pub struct HelloCube;

impl HelloCube {
    fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
        let material = materials.add(StandardMaterial::default());

        let mut mesh = Mesh::from(Cuboid::default());

        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![
            LinearRgba::RED.to_f32_array(), LinearRgba::GREEN.to_f32_array(), LinearRgba::BLUE.to_f32_array(), LinearRgba::BLACK.to_f32_array(),
            LinearRgba::RED.to_f32_array(), LinearRgba::GREEN.to_f32_array(), LinearRgba::BLUE.to_f32_array(), LinearRgba::BLACK.to_f32_array(),
            LinearRgba::RED.to_f32_array(), LinearRgba::GREEN.to_f32_array(), LinearRgba::BLUE.to_f32_array(), LinearRgba::BLACK.to_f32_array(),
            LinearRgba::RED.to_f32_array(), LinearRgba::GREEN.to_f32_array(), LinearRgba::BLUE.to_f32_array(), LinearRgba::BLACK.to_f32_array(),
            LinearRgba::RED.to_f32_array(), LinearRgba::GREEN.to_f32_array(), LinearRgba::BLUE.to_f32_array(), LinearRgba::BLACK.to_f32_array(),
            LinearRgba::RED.to_f32_array(), LinearRgba::GREEN.to_f32_array(), LinearRgba::BLUE.to_f32_array(), LinearRgba::BLACK.to_f32_array(),
        ]);

        let cube = meshes.add(mesh);

        let mut transform = Transform::from_xyz(0.0, 1.0, 0.0);
        transform.rotate_local_y(45.0);

        commands.spawn((
            Mesh3d(cube),
            MeshMaterial3d(material),
            transform,
        ));

        commands.spawn((
            PointLight {
                intensity: 10_000_000.,
                range: 100.0,
                shadow_depth_bias: 0.2,
                ..default()
            },
            Transform::from_xyz(8.0, 16.0, 8.0),
        ));

        commands.spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 7., 14.0).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
        ));
    }
}

impl Plugin for HelloCube {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, Self::setup);
    }
}