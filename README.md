# Orion-Engine

Orion is a Rust engine for rendering signed distance fields (SDFs) and triangles. The starter renderer uses an ash Vulkan compute pipeline to sphere-trace SDFs and intersect triangles along the same rays, then presents the result in a winit window.

## Run the demo

Use a recent stable Rust toolchain and a Vulkan 1.1 driver with graphics, compute, and swapchain support. The demo needs a graphics and compute queue and an 8-bit RGBA or BGRA swapchain that supports transfers and color attachments. Cargo compiles the WGSL shader to SPIR-V at build time using Naga.

```sh
cargo run
```

The demo renders three spheres, the bundled Suzanne OBJ, and the chess set from `scene.glb` over a checkerboard plane, with a green triangle passing through the red sphere. Triangles are visible from the side where their vertices wind counterclockwise. The resizable window starts at a logical size of 960 × 540. The egui overlay in the top-left corner shows presented frames per second (FPS) and completed simulation ticks per second (TPS), sampled over roughly half-second intervals.

Pass an OBJ or GLB file to add its geometry to the demo scene. Orion triangulates OBJ polygon faces and uses the embedded geometry from a GLB's default scene, falling back to the first scene or to untransformed meshes if the asset has no scenes. GLB node transforms are baked into the imported triangles. Both formats use a uniform color and flat face normals. Prepare the mesh for a Y-up scene and place it in view of the demo camera.

```sh
cargo run -- path/to/mesh.obj
cargo run -- path/to/mesh.glb
```

The demo prefers Vulkan's immediate present mode for uncapped FPS measurements. It uses mailbox mode when immediate mode is unavailable and falls back to refresh-synced FIFO when the surface offers neither mode.

Click inside the window to capture the mouse for freecam. Move the mouse to look around, use **W/A/S/D** to fly, and use **Space/Ctrl** to rise or descend. Hold **Shift** to move faster. Press **Esc** to release the cursor.

The simulation advances at a fixed 60 TPS. Freecam key changes and mouse deltas are queued for the next tick when the event loop receives them. The gold sphere bobs vertically on a two-second sine wave driven by simulation ticks. Rendering blends the camera and sphere positions between the two latest completed states, one tick behind real time. Given the same starting state and ordered inputs per tick, the simulation produces the same state regardless of redraw timing.

## Save and resume

Press **F5** to save the latest completed tick to `orion-quicksave.json` in the working directory. Press **F9** to reload it. Both shortcuts work with the cursor captured or released and report the tick in the terminal. Copy the file before the next quicksave to keep multiple checkpoints. Use `--load` to open any checkpoint when launching the demo.

```sh
cargo run -- --load orion-quicksave.json
```

Checkpoints include the active camera, scene geometry, animation state, held controls, and queued input. Meshes are embedded in the file. Press **Esc** after loading if you want to release saved movement controls. The `snapshot` command runs the demo without input to an exact tick and saves it without opening a window. Add `--mesh path/to/mesh.glb` or `--mesh path/to/mesh.obj` to include a custom mesh. The existing `--obj` flag is an alias for `--mesh`.

```sh
cargo run -- snapshot tick-1000.json --tick 1000
```

## Render video

Offline export requires a Vulkan 1.1 compute device and an FFmpeg build with `libx264` on `PATH`. It renders the scene without opening a window and writes an H.264 MP4. Output defaults to 960 × 540 and 60 FPS. Set `--width` and `--height` to positive even values to change the resolution.

```sh
cargo run -- render tick-1000.json output.mp4 --ticks 100 --tps 20 --fps 60
```

This produces 300 frames over five seconds. The first frame shows tick 1000, and the final frame blends ticks 1099 and 1100. `--tps` sets how many simulation ticks play per video second and defaults to 60. Simulation steps still use the fixed 1/60-second timestep. When the requested duration falls between video frame boundaries, export rounds the frame count up.

## Workspace

| Crate | Responsibility |
| --- | --- |
| [`orion-scene`](crates/orion-scene) | [hecs](https://github.com/Ralith/hecs) world, scene components, OBJ and GLB import, and demo setup. |
| [`orion-renderer`](crates/orion-renderer) | Vulkan device, compute pipeline, scene upload, swapchain presentation, and offscreen rendering. The compute shader is assembled from the files in [`shaders`](crates/orion-renderer/shaders). |
| [`orion-app`](crates/orion-app) | Default executable. It drives the demo, checkpoints, and video export. |

`Scene::world` stores cameras, spheres, and meshes as components. Set `Scene::active_camera` to select the camera entity. The renderer queries the world each frame, so spawning or despawning a sphere or mesh affects the next frame. `Scene::import_obj` and `Scene::import_glb` return the entity for the imported mesh.

Run the workspace tests with `cargo test --workspace`. They run without a Vulkan device or a window.
