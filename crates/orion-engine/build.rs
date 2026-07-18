use spirv_builder::{ModuleResult, SpirvBuilder};
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();

    let shader_crate = workspace_root.join("crates").join("orion-shader");

    let mut builder = SpirvBuilder::new(shader_crate, "spirv-unknown-vulkan1.3");

    builder.build_script.defaults = true;
    builder.multimodule = true;

    let compile_result = builder.build()?;

    let assets_dir = workspace_root.join("assets").join("shaders").join("generated");

    if assets_dir.exists() {
        std::fs::remove_dir_all(&assets_dir)?;
    }

    std::fs::create_dir_all(&assets_dir)?;

    match compile_result.module {
        ModuleResult::SingleModule(spv_path) => {
            let destination = assets_dir.join("shader.spv");

            std::fs::copy(spv_path, destination)?;
        }
        ModuleResult::MultiModule(modules) => {
            for (binary_name, spv_path) in modules {
                let file_name = format!("{}.spv", binary_name);
                let destination = assets_dir.join(file_name);

                std::fs::copy(spv_path, destination)?;
            }
        }
    }

    Ok(())
}