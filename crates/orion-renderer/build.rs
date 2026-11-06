use std::{env, fs, path::PathBuf};

use naga::back::spv;
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn main() {
    let shader = "shaders/raymarch.wgsl";
    println!("cargo:rerun-if-changed={shader}");
    let source = fs::read_to_string(shader).expect("read raymarch shader");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    let options = spv::Options {
        lang_version: (1, 3),
        ..Default::default()
    };
    let words = spv::write_vec(&module, &info, &options, None).expect("generate raymarch SPIR-V");
    let bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build output directory"));
    fs::write(out.join("raymarch.spv"), bytes).expect("write raymarch SPIR-V");
}
