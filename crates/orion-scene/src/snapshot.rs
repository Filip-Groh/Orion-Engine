use anyhow::{Context, Result, ensure};
use hecs::{EntityBuilder, EntityRef};
use serde::{Deserialize, Serialize};

use crate::{Camera, Entity, Mesh, Scene, SineBob, Sphere, Texture, World};

/// A self-contained copy of a scene. References use indices into the saved entities.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSnapshot {
    active_camera: usize,
    entities: Vec<SavedEntity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    textures: Vec<Texture>,
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
            if entity.entity() == scene.active_camera {
                active_camera = Some(entities.len());
            }
            entities.push(SavedEntity::capture(entity)?);
        }
        Ok(Self {
            active_camera: active_camera.context("active camera was missing from the snapshot")?,
            entities,
            textures: scene.textures.clone(),
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
            let handle = entity.spawn(&mut world, &mut builder);
            if index == self.active_camera {
                active_camera = Some(handle);
            }
        }
        Ok(Scene {
            world,
            textures: self.textures,
            active_camera: active_camera.expect("active camera index was validated"),
        })
    }
}

impl SavedEntity {
    fn capture(entity: EntityRef<'_>) -> Result<Self> {
        let saved = Self {
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
        Ok(saved)
    }

    fn spawn(self, world: &mut World, builder: &mut EntityBuilder) -> Entity {
        if let Some(camera) = self.camera {
            builder.add(camera);
        }
        if let Some(sphere) = self.sphere {
            builder.add(sphere);
        }
        if let Some(bob) = self.sine_bob {
            builder.add(bob);
        }
        if let Some(mesh) = self.mesh {
            builder.add(mesh);
        }
        world.spawn(builder.build())
    }
}
