use std::f32::consts::{FRAC_PI_4, PI};
use bevy::app::{App, Plugin, Startup};
use bevy::asset::Assets;
use bevy::color::Color;
use bevy::color::palettes::tailwind;
use bevy::light::PointLight;
use bevy::math::{EulerRot, Quat, Vec2, Vec3};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::{default, Commands, Cuboid, Plane3d, ResMut, Sphere, Transform};

pub struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_lights, spawn_world));
    }
}

fn spawn_lights(mut commands: Commands) {
    // Main light
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::ORANGE_300),
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, 0.0),
    ));
    // Light behind wall
    commands.spawn((
        PointLight {
            color: Color::WHITE,
            ..default()
        },
        Transform::from_xyz(-3.5, 3.0, 0.0),
    ));
    // Light under floor
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::RED_300),
            ..default()
        },
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
}

fn spawn_world(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let floor = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(10.0)));
    let sphere = meshes.add(Sphere::new(0.5));
    let wall = meshes.add(Cuboid::new(0.2, 4.0, 3.0));

    let blue_material = materials.add(Color::from(tailwind::BLUE_700));
    let red_material = materials.add(Color::from(tailwind::RED_950));
    let white_material = materials.add(Color::WHITE);

    // Top side of floor
    commands.spawn((
        Mesh3d(floor.clone()),
        MeshMaterial3d(white_material.clone()),
    ));
    // Under side of floor
    commands.spawn((
        Mesh3d(floor.clone()),
        MeshMaterial3d(white_material.clone()),
        Transform::from_xyz(0.0, -0.01, 0.0).with_rotation(Quat::from_rotation_x(PI)),
    ));
    // Blue sphere
    commands.spawn((
        Mesh3d(sphere.clone()),
        MeshMaterial3d(blue_material.clone()),
        Transform::from_xyz(3.0, 1.5, 0.0),
    ));
    // Tall wall
    commands.spawn((
        Mesh3d(wall.clone()),
        MeshMaterial3d(white_material.clone()),
        Transform::from_xyz(-3.0, 2.0, 0.0),
    ));
    // Cube behind wall
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(blue_material.clone()),
        Transform::from_xyz(-4.2, 0.5, 0.0),
    ));
    // Hidden cube under floor
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(red_material.clone()),
        Transform {
            translation: Vec3::new(3.0, -2.0, 0.0),
            rotation: Quat::from_euler(EulerRot::YXZEx, FRAC_PI_4, FRAC_PI_4, 0.0),
            ..default()
        },
    ));
}