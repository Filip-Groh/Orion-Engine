use anyhow::{Context, Result, ensure};
use hecs::EntityBuilder;
use serde::{Deserialize, Serialize};

use crate::{Camera, Mesh, Scene, SineBob, Sphere, World};

/// A self-contained copy of a scene. References use indices into the saved entities.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSnapshot {
    active_camera: usize,
    entities: Vec<SavedEntity>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedEntity {
    camera: Option<Camera>,
    sphere: Option<Sphere>,
    sine_bob: Option<SineBob>,
    mesh: Option<Mesh>,
}

impl SceneSnapshot {
    pub fn capture(scene: &Scene) -> Result<Self> {
        scene.camera()?;
        let mut active_camera = None;
        let mut entities = Vec::with_capacity(scene.world.len() as usize);
        for entity in scene.world.iter() {
            let saved = SavedEntity {
                camera: entity.get::<&Camera>().as_deref().copied(),
                sphere: entity.get::<&Sphere>().as_deref().copied(),
                sine_bob: entity.get::<&SineBob>().as_deref().copied(),
                mesh: entity.get::<&Mesh>().as_deref().cloned(),
            };
            let component_count = [
                saved.camera.is_some(),
                saved.sphere.is_some(),
                saved.sine_bob.is_some(),
                saved.mesh.is_some(),
            ]
            .into_iter()
            .filter(|present| *present)
            .count();
            ensure!(
                component_count == entity.len(),
                "entity {:?} has a component that checkpoints do not support",
                entity.entity()
            );
            if entity.entity() == scene.active_camera {
                active_camera = Some(entities.len());
            }
            entities.push(saved);
        }
        Ok(Self {
            active_camera: active_camera.context("active camera was missing from the snapshot")?,
            entities,
        })
    }

    pub fn restore(self) -> Result<Scene> {
        ensure!(
            self.entities
                .get(self.active_camera)
                .is_some_and(|entity| entity.camera.is_some()),
            "checkpoint must identify an active camera"
        );
        let mut world = World::new();
        let mut builder = EntityBuilder::new();
        let mut active_camera = None;
        for (index, entity) in self.entities.into_iter().enumerate() {
            if let Some(camera) = entity.camera {
                builder.add(camera);
            }
            if let Some(sphere) = entity.sphere {
                builder.add(sphere);
            }
            if let Some(bob) = entity.sine_bob {
                builder.add(bob);
            }
            if let Some(mesh) = entity.mesh {
                builder.add(mesh);
            }
            let handle = world.spawn(builder.build());
            if index == self.active_camera {
                active_camera = Some(handle);
            }
        }
        Ok(Scene {
            world,
            active_camera: active_camera.expect("active camera index was validated"),
        })
    }
}
