use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

/// Marker component to flag an entity as our controllable free camera
#[derive(Component)]
pub struct FreeCam {
    pub speed: f32,
    pub sensitivity: f32,
}

impl Default for FreeCam {
    fn default() -> Self {
        Self {
            speed: 20.0,       // Units per second
            sensitivity: 0.15, // Mouse look speed multiplier
        }
    }
}

fn update_free_cam(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    mut query: Query<(&mut Transform, &FreeCam)>,
) {
    let delta_time = time.delta_secs();

    for (mut transform, config) in query.iter_mut() {
        // --- Part 1: Handle Rotation (Mouse Look) ---
        let mut mouse_delta = Vec2::ZERO;
        for event in mouse_motion_events.read() {
            mouse_delta += event.delta;
        }

        if mouse_delta != Vec2::ZERO {
            // Apply sensitivity and convert to radians
            let yaw = -mouse_delta.x * config.sensitivity * 1.0_f32.to_radians();
            let pitch = -mouse_delta.y * config.sensitivity * 1.0_f32.to_radians();

            // Rotate globally around the Y axis for left/right look (prevents head tilt)
            transform.rotate_y(yaw);

            // Rotate locally for up/down look
            transform.rotate_local_x(pitch);

            // Clamp pitch to prevent flipping completely upside down (-89 to 89 degrees)
            let mut angles = transform.rotation.to_euler(EulerRot::YXZ);
            angles.1 = angles.1.clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());
            transform.rotation = Quat::from_euler(EulerRot::YXZ, angles.0, angles.1, 0.0);
        }

        // --- Part 2: Handle Translation (Keyboard Movement) ---
        let mut direction = Vec3::ZERO;

        // Bevy's forward/right/up returns a Dir3. Convert explicitly to Vec3.
        let forward = Vec3::from(transform.forward());
        let right = Vec3::from(transform.right());
        let up = Vec3::from(transform.up());

        if keys.pressed(KeyCode::KeyW) { direction += forward; }
        if keys.pressed(KeyCode::KeyS) { direction -= forward; }
        if keys.pressed(KeyCode::KeyA) { direction -= right; }
        if keys.pressed(KeyCode::KeyD) { direction += right; }
        if keys.pressed(KeyCode::Space) { direction += up; }      // Fly Up
        if keys.pressed(KeyCode::ShiftLeft) { direction -= up; } // Fly Down

        // Prevent diagonal movement from being faster
        if direction != Vec3::ZERO {
            transform.translation += direction.normalize() * config.speed * delta_time;
        }
    }
}

fn handle_cursor_lock(
    // In Bevy 0.15+, CursorOptions is its own component on the Window entity.
    // Single<&mut T> guarantees retrieving the unique primary window component cleanly.
    mut cursor_options: Single<&mut CursorOptions>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) {
        cursor_options.grab_mode = CursorGrabMode::Locked;
        cursor_options.visible = false;
    }

    if keys.just_pressed(KeyCode::Escape) {
        cursor_options.grab_mode = CursorGrabMode::None;
        cursor_options.visible = true;
    }
}

// Plugin that spawns the camera.
pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
        app.add_systems(Update, (update_free_cam, handle_cursor_lock));
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCam::default(),
    ));
}