use std::{borrow::Cow, collections::HashMap, fs, path::Path};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use gltf::{
    image::Source,
    texture::{Info, MagFilter, MinFilter, WrappingMode},
};
use percent_encoding::percent_decode_str;

use crate::{Texture, WrapMode};

pub(super) struct Textures<'a> {
    pub bin: &'a [u8],
    directory: Option<&'a Path>,
    base: usize,
    indices: HashMap<usize, u32>,
    pub images: Vec<Texture>,
}

impl<'a> Textures<'a> {
    pub fn new(bin: &'a [u8], directory: Option<&'a Path>, base: usize) -> Self {
        Self {
            bin,
            directory,
            base,
            indices: HashMap::new(),
            images: Vec::new(),
        }
    }

    pub fn load(&mut self, info: &Info<'_>) -> Result<u32> {
        let texture = info.texture();
        if let Some(&index) = self.indices.get(&texture.index()) {
            return Ok(index);
        }
        let sampler = texture.sampler();
        let image = texture.source();
        let mut decoded = self
            .decode(image.source())
            .with_context(|| format!("load GLB image {}", image.index()))?;
        decoded.wrap_u = wrap(sampler.wrap_s());
        decoded.wrap_v = wrap(sampler.wrap_t());
        decoded.linear_filter = match sampler.mag_filter() {
            Some(filter) => filter == MagFilter::Linear,
            None => !matches!(
                sampler.min_filter(),
                Some(
                    MinFilter::Nearest
                        | MinFilter::NearestMipmapNearest
                        | MinFilter::NearestMipmapLinear
                )
            ),
        };
        let index = (self.base + self.images.len()).try_into()?;
        self.images.push(decoded);
        self.indices.insert(texture.index(), index);
        Ok(index)
    }

    fn decode(&self, source: Source<'_>) -> Result<Texture> {
        let bytes = match source {
            Source::View { view, .. } => {
                super::validate_range(view.clone(), 0, 1, view.length(), self.bin.len())?;
                Cow::Borrowed(&self.bin[view.offset()..][..view.length()])
            }
            Source::Uri { uri, .. } => Cow::Owned(self.read_uri(uri)?),
        };
        Texture::decode(&bytes)
    }

    fn read_uri(&self, uri: &str) -> Result<Vec<u8>> {
        if let Some(data) = uri.strip_prefix("data:") {
            let (header, body) = data.split_once(',').context("invalid image data URI")?;
            let body: Vec<_> = percent_decode_str(body).collect();
            return if header.ends_with(";base64") {
                Ok(STANDARD.decode(body)?)
            } else {
                Ok(body)
            };
        }
        let directory = self
            .directory
            .context("embedded GLB cannot resolve external images")?;
        let path = percent_decode_str(uri).decode_utf8()?;
        ensure!(
            !path.contains(':') && Path::new(path.as_ref()).is_relative(),
            "GLB image URI must be a relative path or data URI"
        );
        let path = directory.join(path.as_ref());
        fs::read(&path).with_context(|| format!("read {}", path.display()))
    }
}

fn wrap(mode: WrappingMode) -> WrapMode {
    match mode {
        WrappingMode::Repeat => WrapMode::Repeat,
        WrappingMode::ClampToEdge => WrapMode::Clamp,
        WrappingMode::MirroredRepeat => WrapMode::Mirror,
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct UvSet {
    pub index: u32,
    offset: [f32; 2],
    scale: [f32; 2],
    rotation: f32,
}

impl UvSet {
    pub fn new(info: Option<&Info<'_>>) -> Self {
        let index = info.map_or(0, Info::tex_coord);
        if let Some(transform) = info.and_then(Info::texture_transform) {
            Self {
                index: transform.tex_coord().unwrap_or(index),
                offset: transform.offset(),
                scale: transform.scale(),
                rotation: transform.rotation(),
            }
        } else {
            Self {
                index,
                offset: [0.0; 2],
                scale: [1.0; 2],
                rotation: 0.0,
            }
        }
    }

    pub fn apply(self, [u, v]: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.rotation.sin_cos();
        let x = self.scale[0] * u;
        let y = self.scale[1] * v;
        [
            self.offset[0] + cos * x - sin * y,
            self.offset[1] + sin * x + cos * y,
        ]
    }
}
