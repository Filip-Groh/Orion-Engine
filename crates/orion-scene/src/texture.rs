use std::io::Cursor;

use anyhow::{Context, Result};
use image::ImageReader;
use serde::{Deserialize, Serialize};

use crate::{DEFAULT_ROUGHNESS, Mesh, Triangle};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum WrapMode {
    #[default]
    Repeat = 0,
    Clamp = 1,
    Mirror = 2,
}

/// RGBA8 texels in row order, starting at the upper-left corner.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    #[serde(with = "texel_bytes")]
    pub texels: Vec<[u8; 4]>,
    pub wrap_u: WrapMode,
    pub wrap_v: WrapMode,
    pub linear_filter: bool,
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

impl Texture {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        let pixels = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()?
            .decode()
            .context("decode PNG or JPEG texture")?
            .into_rgba8();
        let (width, height) = pixels.dimensions();
        let texels = pixels.into_raw().as_chunks::<4>().0.to_vec();
        Ok(Self {
            width,
            height,
            texels,
            wrap_u: WrapMode::Repeat,
            wrap_v: WrapMode::Repeat,
            linear_filter: true,
            scale: [1.0; 2],
            offset: [0.0; 2],
        })
    }
}

/// Indices into `Scene::textures`. Scalar MTL maps use red, and glTF's combined map uses G/B.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialMaps {
    pub base_color: Option<u32>,
    pub metallic_roughness: Option<u32>,
    pub metallic: Option<u32>,
    pub roughness: Option<u32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TriangleUvs {
    /// UVs in vertex order. V increases down the texture.
    pub primary: [[f32; 2]; 3],
    /// Separate coordinates for the combined metallic-roughness map, when supplied by glTF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_roughness: Option<[[f32; 2]; 3]>,
    pub maps: MaterialMaps,
}

#[derive(Clone, Copy)]
pub(crate) struct ImportedMaterial {
    pub albedo: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub maps: MaterialMaps,
}

impl ImportedMaterial {
    pub fn new(albedo: [f32; 3]) -> Self {
        Self {
            albedo,
            metallic: 0.0,
            roughness: DEFAULT_ROUGHNESS,
            maps: MaterialMaps::default(),
        }
    }
}

impl Mesh {
    pub(crate) fn push_triangle(
        &mut self,
        vertices: [[f32; 3]; 3],
        material: ImportedMaterial,
        primary_uvs: Option<[[f32; 2]; 3]>,
        metallic_roughness: Option<[[f32; 2]; 3]>,
    ) -> Result<()> {
        let mut uv_index = None;
        if let Some(primary) = primary_uvs {
            uv_index = Some(self.uvs.len().try_into()?);
            self.uvs.push(TriangleUvs {
                primary,
                metallic_roughness,
                maps: material.maps,
            });
        }
        self.triangles.push(Triangle {
            vertices,
            albedo: material.albedo,
            metallic: material.metallic,
            roughness: material.roughness,
            uv_index,
        });
        Ok(())
    }
}

mod texel_bytes {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(texels: &[[u8; 4]], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(texels.as_flattened()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<[u8; 4]>, D::Error> {
        let bytes = STANDARD
            .decode(String::deserialize(deserializer)?)
            .map_err(D::Error::custom)?;
        if !bytes.len().is_multiple_of(4) {
            return Err(D::Error::custom("texture must contain whole RGBA texels"));
        }
        Ok(bytes.as_chunks::<4>().0.to_vec())
    }
}
