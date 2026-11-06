# Orion-Engine

Orion is a Rust engine for rendering signed distance fields (SDFs) and triangles. The starter renderer uses an ash Vulkan compute pipeline to sphere-trace SDFs and intersect triangles along the same rays, then presents the result in a winit window.

## Run the demo

Use a recent stable Rust toolchain and a Vulkan 1.1 driver with graphics, compute, and swapchain support. The demo needs a graphics and compute queue and an 8-bit RGBA or BGRA swapchain that supports transfers and color attachments. Cargo compiles the WGSL shader to SPIR-V at build time using Naga.

```sh
cargo run
```

The demo renders three spheres over a checkerboard plane, with a green triangle passing through the red sphere. The resizable window starts at a logical size of 960 × 540. The egui overlay in the top-left corner shows the frame rate, averaged over half-second intervals.

The demo prefers Vulkan's immediate present mode for uncapped FPS measurements. It uses mailbox mode when immediate mode is unavailable and falls back to refresh-synced FIFO when the surface offers neither mode.

Click inside the window to capture the mouse for freecam. Move the mouse to look around, use **W/A/S/D** to fly, and use **Space/Ctrl** to rise or descend. Hold **Shift** to move faster. Press **Esc** to release the cursor.

## Workspace

| Crate | Responsibility |
| --- | --- |
| [`orion-scene`](crates/orion-scene) | Camera, sphere and triangle parameters, and the demo scene. |
| [`orion-renderer`](crates/orion-renderer) | Vulkan device, compute pipeline, scene upload, and swapchain presentation. The ray marcher and shading live in [`raymarch.wgsl`](crates/orion-renderer/shaders/raymarch.wgsl). |
| [`orion-app`](crates/orion-app) | Default executable. It owns the winit event loop and drives the demo. |
