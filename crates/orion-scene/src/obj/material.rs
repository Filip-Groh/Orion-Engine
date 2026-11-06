use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};

use crate::{MaterialMaps, Texture, WrapMode, texture::ImportedMaterial};

use super::statement;

pub(super) struct MaterialLibrary {
    definitions: HashMap<String, Definition>,
    materials: HashMap<String, ImportedMaterial>,
    fallback: ImportedMaterial,
    texture_base: usize,
    loaded_maps: Vec<Map>,
    pub textures: Vec<Texture>,
}

#[derive(Default)]
struct Definition {
    albedo: Option<[f32; 3]>,
    metallic: Option<f32>,
    roughness: Option<f32>,
    shininess: Option<f32>,
    base_map: Option<Map>,
    metallic_map: Option<Map>,
    roughness_map: Option<Map>,
}

#[derive(PartialEq)]
struct Map {
    path: PathBuf,
    wrap: WrapMode,
    scale: [f32; 2],
    offset: [f32; 2],
}

impl MaterialLibrary {
    pub(super) fn new(fallback: ImportedMaterial, texture_base: usize) -> Self {
        Self {
            definitions: HashMap::new(),
            materials: HashMap::new(),
            fallback,
            texture_base,
            loaded_maps: Vec::new(),
            textures: Vec::new(),
        }
    }

    pub(super) fn load(&mut self, directory: &Path, files: &str) -> Result<()> {
        // A single filename may include spaces. Multiple libraries use separate words.
        if directory.join(files).is_file() {
            return self.load_file(&directory.join(files));
        }
        for file in files.split_whitespace() {
            self.load_file(&directory.join(file))?;
        }
        Ok(())
    }

    fn load_file(&mut self, path: &Path) -> Result<()> {
        let source =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let directory = path.parent().unwrap_or(Path::new(""));
        let mut active = None;
        for (index, (keyword, rest)) in source
            .trim_start_matches('\u{feff}')
            .lines()
            .map(statement)
            .enumerate()
        {
            if keyword == "newmtl" {
                active = Some(self.definitions.entry(rest.to_owned()).or_default());
            } else if let Some(definition) = active.as_deref_mut() {
                definition
                    .read(keyword, rest, directory)
                    .with_context(|| format!("{} line {}", path.display(), index + 1))?;
            }
        }
        Ok(())
    }

    pub(super) fn resolve(&mut self, name: &str) -> Result<ImportedMaterial> {
        if let Some(&material) = self.materials.get(name) {
            return Ok(material);
        }
        let Some(definition) = self.definitions.remove(name) else {
            return Ok(self.fallback);
        };
        let maps = MaterialMaps {
            base_color: self.load_map(definition.base_map)?,
            metallic: self.load_map(definition.metallic_map)?,
            roughness: self.load_map(definition.roughness_map)?,
            ..MaterialMaps::default()
        };
        let albedo = definition.albedo.unwrap_or(if maps.base_color.is_some() {
            [1.0; 3]
        } else {
            self.fallback.albedo
        });
        let metallic =
            definition
                .metallic
                .unwrap_or(if maps.metallic.is_some() { 1.0 } else { 0.0 });
        let roughness = definition.roughness.unwrap_or_else(|| {
            if maps.roughness.is_some() {
                1.0
            } else if let Some(shininess) = definition.shininess {
                (2.0 / (shininess + 2.0)).powf(0.25)
            } else {
                self.fallback.roughness
            }
        });
        let material = ImportedMaterial {
            albedo,
            metallic,
            roughness,
            maps,
        };
        self.materials.insert(name.to_owned(), material);
        Ok(material)
    }

    fn load_map(&mut self, map: Option<Map>) -> Result<Option<u32>> {
        let Some(map) = map else {
            return Ok(None);
        };
        let index = if let Some(index) = self.loaded_maps.iter().position(|loaded| *loaded == map) {
            index
        } else {
            let texture = (|| {
                let mut texture = Texture::decode(&fs::read(&map.path)?)?;
                texture.wrap_u = map.wrap;
                texture.wrap_v = map.wrap;
                texture.scale = map.scale;
                texture.offset = [map.offset[0], 1.0 - map.scale[1] - map.offset[1]];
                Ok::<_, anyhow::Error>(texture)
            })()
            .with_context(|| format!("load texture {}", map.path.display()))?;
            let index = self.textures.len();
            self.textures.push(texture);
            self.loaded_maps.push(map);
            index
        };
        Ok(Some((self.texture_base + index).try_into()?))
    }
}

impl Definition {
    fn read(&mut self, keyword: &str, rest: &str, directory: &Path) -> Result<()> {
        let mut fields = rest.split_whitespace();
        match keyword {
            "Kd" => {
                let r = number(fields.next())?.clamp(0.0, 1.0);
                let g = fields
                    .next()
                    .map(|value| number(Some(value)))
                    .transpose()?
                    .unwrap_or(r);
                let b = fields
                    .next()
                    .map(|value| number(Some(value)))
                    .transpose()?
                    .unwrap_or(r);
                self.albedo = Some([r, g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)]);
            }
            "Pm" => self.metallic = Some(number(fields.next())?.clamp(0.0, 1.0)),
            "Pr" => self.roughness = Some(number(fields.next())?.clamp(0.0, 1.0)),
            "Ns" => self.shininess = Some(number(fields.next())?.max(0.0)),
            "map_Kd" => self.base_map = Some(Map::parse(rest, directory)?),
            "map_Pm" => self.metallic_map = Some(Map::parse(rest, directory)?),
            "map_Pr" => self.roughness_map = Some(Map::parse(rest, directory)?),
            _ => {}
        }
        Ok(())
    }
}

impl Map {
    fn parse(mut rest: &str, directory: &Path) -> Result<Self> {
        let mut wrap = WrapMode::Repeat;
        let mut scale = [1.0; 2];
        let mut offset = [0.0; 2];
        while rest.starts_with('-') {
            let (option, tail) = statement(rest);
            rest = tail;
            match option {
                "-clamp" => {
                    let (mode, tail) = statement(rest);
                    wrap = match mode {
                        "on" => WrapMode::Clamp,
                        "off" => WrapMode::Repeat,
                        _ => bail!("-clamp requires on or off"),
                    };
                    rest = tail;
                }
                "-s" | "-o" => {
                    let values = if option == "-s" {
                        &mut scale
                    } else {
                        &mut offset
                    };
                    let (first, tail) = statement(rest);
                    values[0] = number(Some(first))?;
                    rest = tail;
                    for component in 1..3 {
                        let (value, tail) = statement(rest);
                        let Ok(value) = value.parse::<f32>() else {
                            break;
                        };
                        ensure!(value.is_finite(), "texture transform must be finite");
                        if component == 1 {
                            values[1] = value;
                        }
                        rest = tail;
                    }
                }
                _ => bail!("unsupported texture map option {option}"),
            }
        }
        ensure!(!rest.is_empty(), "texture map requires a filename");
        let path = directory.join(rest.trim_matches('"'));
        Ok(Self {
            path,
            wrap,
            scale,
            offset,
        })
    }
}

fn number(value: Option<&str>) -> Result<f32> {
    let value: f32 = value.context("missing material value")?.parse()?;
    ensure!(value.is_finite(), "material value must be finite");
    Ok(value)
}
