use std::{env, fs, path::PathBuf};

use naga::back::spv;
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn main() {
    let source = load_shader();
    let bytes = compile_shader(&source);
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build output directory"));
    fs::write(out.join("raymarch.spv"), bytes).expect("write raymarch SPIR-V");
}

fn load_shader() -> String {
    let mut source = String::new();
    // WGSL has a single module here, so concatenate its source files in order.
    for path in [
        "shaders/scene.wgsl",
        "shaders/mesh.wgsl",
        "shaders/raymarch.wgsl",
    ] {
        println!("cargo:rerun-if-changed={path}");
        let part = fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
        source.push_str(&part);
        source.push('\n');
    }
    source
}

fn compile_shader(source: &str) -> Vec<u8> {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    let options = spv::Options {
        lang_version: (1, 3),
        ..Default::default()
    };
    let words = spv::write_vec(&module, &info, &options, None).expect("generate raymarch SPIR-V");
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}
