# Orion-Engine

Orion is a Rust engine for rendering signed distance fields (SDFs) and triangles. The starter renderer uses an ash Vulkan compute pipeline to sphere-trace SDFs and intersect triangles along the same rays, then presents the result in a winit window.

## Run the demo

Use a recent stable Rust toolchain and a Vulkan 1.1 driver with graphics, compute, and swapchain support. The demo needs a graphics and compute queue and an 8-bit RGBA or BGRA swapchain that supports transfers and color attachments. Cargo compiles the WGSL shader to SPIR-V at build time using Naga.

```sh
cargo run
```

The demo renders three spheres and the bundled Suzanne OBJ over a checkerboard plane, with a green triangle passing through the red sphere. Triangles are visible from the side where their vertices wind counterclockwise. The resizable window starts at a logical size of 960 × 540. The egui overlay in the top-left corner shows the frame rate, averaged over half-second intervals.

Pass an OBJ file to add it to the demo scene in its original coordinates. Orion triangulates polygon faces and shades them with a uniform color and flat face normals. Prepare the mesh for a Y-up scene and place it in view of the demo camera.

```sh
cargo run -- path/to/mesh.obj
```

The demo prefers Vulkan's immediate present mode for uncapped FPS measurements. It uses mailbox mode when immediate mode is unavailable and falls back to refresh-synced FIFO when the surface offers neither mode.

Click inside the window to capture the mouse for freecam. Move the mouse to look around, use **W/A/S/D** to fly, and use **Space/Ctrl** to rise or descend. Hold **Shift** to move faster. Press **Esc** to release the cursor.

## Workspace

| Crate | Responsibility |
| --- | --- |
| [`orion-scene`](crates/orion-scene) | [hecs](https://github.com/Ralith/hecs) world, scene components, OBJ import, and demo setup. |
| [`orion-renderer`](crates/orion-renderer) | Vulkan device, compute pipeline, scene upload, and swapchain presentation. The compute shader is assembled from the files in [`shaders`](crates/orion-renderer/shaders). |
| [`orion-app`](crates/orion-app) | Default executable. It owns the winit event loop and drives the demo. |

`Scene::world` stores cameras, spheres, and meshes as components. Set `Scene::active_camera` to select the camera entity. The renderer queries the world each frame, so spawning or despawning a sphere or mesh affects the next frame. `Scene::import_obj` returns the entity for the imported mesh.

Run the workspace tests with `cargo test --workspace`. They run without a Vulkan device or a window.
