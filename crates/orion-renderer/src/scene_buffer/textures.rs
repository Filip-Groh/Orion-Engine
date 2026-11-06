use anyhow::{Result, ensure};
use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use orion_scene::{MaterialMaps, Texture, TriangleUvs};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuUvs {
    primary_ab: [[f32; 2]; 2],
    primary_c_secondary_a: [[f32; 2]; 2],
    secondary_bc: [[f32; 2]; 2],
    maps: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuTexture {
    // Width, height, first texel (in u32s), and sampler flags.
    info: [u32; 4],
    scale_offset: [f32; 4],
}

pub(super) struct TextureLayout<'a> {
    textures: &'a [Texture],
    start: usize,
    texel_start: usize,
    pub end: usize,
}

impl<'a> TextureLayout<'a> {
    pub fn new(textures: &'a [Texture], start: usize) -> Result<Self> {
        let texel_start =
            start.saturating_add(textures.len().saturating_mul(size_of::<GpuTexture>()));
        let mut end = texel_start;
        for texture in textures {
            ensure!(
                texture.width != 0
                    && texture.height != 0
                    && u64::from(texture.width) * u64::from(texture.height)
                        == texture.texels.len() as u64,
                "texture dimensions must match its RGBA texels"
            );
            ensure!(
                texture
                    .scale
                    .into_iter()
                    .chain(texture.offset)
                    .all(f32::is_finite),
                "texture transforms must be finite"
            );
            end = end.saturating_add(texture.texels.len().saturating_mul(4));
        }
        Ok(Self {
            textures,
            start,
            texel_start,
            end,
        })
    }

    pub fn pack_uvs(&self, uvs: TriangleUvs) -> Result<Option<GpuUvs>> {
        let [a, b, c] = uvs.primary;
        let [d, e, f] = uvs.metallic_roughness.unwrap_or(uvs.primary);
        ensure!(
            [a, b, c, d, e, f].into_iter().flatten().all(f32::is_finite),
            "triangle texture coordinates must be finite"
        );
        if uvs.maps == MaterialMaps::default() {
            return Ok(None);
        }
        let map_offset = |index: Option<u32>| -> Result<u32> {
            let Some(index) = index else {
                return Ok(0);
            };
            ensure!(
                (index as usize) < self.textures.len(),
                "triangle texture index {index} is out of bounds"
            );
            let offset = self.start + index as usize * size_of::<GpuTexture>();
            Ok((offset / 16).try_into()?)
        };
        Ok(Some(GpuUvs {
            primary_ab: [a, b],
            primary_c_secondary_a: [c, d],
            secondary_bc: [e, f],
            maps: [
                map_offset(uvs.maps.base_color)?,
                map_offset(uvs.maps.metallic_roughness)?,
                map_offset(uvs.maps.metallic)?,
                map_offset(uvs.maps.roughness)?,
            ],
        }))
    }

    pub fn append(&self, bytes: &mut Vec<u8>) -> Result<()> {
        let mut first_texel = self.texel_start / 4;
        for texture in self.textures {
            let [sx, sy] = texture.scale;
            let [ox, oy] = texture.offset;
            // Keep these bits in sync with sample_texture in textures.wgsl.
            let flags = texture.wrap_u as u32
                | ((texture.wrap_v as u32) << 2)
                | (u32::from(texture.linear_filter) << 4);
            bytes.extend_from_slice(bytes_of(&GpuTexture {
                info: [
                    texture.width,
                    texture.height,
                    first_texel.try_into()?,
                    flags,
                ],
                scale_offset: [sx, sy, ox, oy],
            }));
            first_texel += texture.texels.len();
        }
        for texture in self.textures {
            bytes.extend_from_slice(cast_slice(&texture.texels));
        }
        Ok(())
    }
}
