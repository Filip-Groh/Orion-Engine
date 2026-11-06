mod app;
mod freecam;
mod rates;
mod tick;

use std::env;

use anyhow::{Result, ensure};
use orion_scene::Scene;

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let obj_path = args.next();
    ensure!(args.next().is_none(), "usage: orion-app [OBJ path]");
    let mut scene = Scene::demo();
    if let Some(path) = obj_path {
        scene.import_obj(path, [0.65, 0.65, 0.65])?;
    }
    app::App::new(scene)?.run()
}
