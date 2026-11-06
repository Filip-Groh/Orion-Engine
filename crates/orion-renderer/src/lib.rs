//! Windowed Vulkan compute renderer for signed distance fields and triangles.

mod device;
mod swapchain;

use std::io::Cursor;

use anyhow::{Context, Result, ensure};
use ash::vk;
use bytemuck::{Pod, Zeroable, bytes_of};
use orion_scene::Scene;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use device::{ComputeDevice, StorageBuffer};
use swapchain::{PresentImage, Swapchain};

// Matches @workgroup_size in raymarch.wgsl.
const WORKGROUP_SIZE: u32 = 8;
const SHADER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/raymarch.spv"));

// The WGSL geometry array follows the header, with spheres before triangles.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneHeader {
    eye_and_fov_scale: [f32; 4],
    look_at: [f32; 4],
    extent_format: [u32; 4],
    geometry_counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuSphere {
    center_radius: [f32; 4],
    albedo: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuTriangle {
    vertices: [[f32; 4]; 3],
    albedo: [f32; 4],
}

pub struct Renderer<W> {
    swapchain: Option<Swapchain>,
    swapchain_dirty: bool,
    dispatch: Dispatch,
    descriptor_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    device: ComputeDevice,
    window: W,
}

impl<W: HasDisplayHandle + HasWindowHandle> Renderer<W> {
    pub fn new(window: W) -> Result<Self> {
        let device = ComputeDevice::new(&window)?;
        let mut renderer = Self {
            swapchain: None,
            swapchain_dirty: false,
            dispatch: Dispatch::new(&device)?,
            descriptor_layout: vk::DescriptorSetLayout::null(),
            pipeline_layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            device,
            window,
        };
        let device = &renderer.device.handle;
        let bindings = [0, 1].map(|binding| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
        });
        let descriptor_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        renderer.descriptor_layout =
            unsafe { device.create_descriptor_set_layout(&descriptor_info, None) }
                .context("create the ray march descriptor layout")?;

        let layouts = [renderer.descriptor_layout];
        let descriptor_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(renderer.dispatch.descriptor_pool)
            .set_layouts(&layouts);
        renderer.dispatch.set = unsafe { device.allocate_descriptor_sets(&descriptor_info) }
            .context("allocate the ray march descriptor set")?[0];
        let layout_info = vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts);
        renderer.pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None) }
            .context("create the ray march pipeline layout")?;

        let spirv =
            ash::util::read_spv(&mut Cursor::new(SHADER)).context("read ray march SPIR-V")?;
        let shader_info = vk::ShaderModuleCreateInfo::default().code(&spirv);
        let shader = unsafe { device.create_shader_module(&shader_info, None) }
            .context("create the ray march shader module")?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader)
            .name(c"main");
        let pipelines = [vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(renderer.pipeline_layout)];
        let result = unsafe {
            let result =
                device.create_compute_pipelines(vk::PipelineCache::null(), &pipelines, None);
            device.destroy_shader_module(shader, None);
            result
        };
        renderer.pipeline = match result {
            Ok(pipelines) => pipelines[0],
            Err((pipelines, error)) => {
                for pipeline in pipelines {
                    unsafe { device.destroy_pipeline(pipeline, None) };
                }
                return Err(error).context("create the ray march compute pipeline");
            }
        };
        Ok(renderer)
    }

    pub fn window(&self) -> &W {
        &self.window
    }

    /// Renders a frame and returns whether another redraw is needed to present it.
    /// A zero-sized window is skipped. Drop the renderer if a Vulkan operation fails.
    pub fn render(
        &mut self,
        scene: &Scene,
        [width, height]: [u32; 2],
        before_present: impl FnOnce(&W),
    ) -> Result<bool> {
        if width == 0 || height == 0 {
            return Ok(false);
        }
        self.dispatch.wait()?;
        let requested_extent = vk::Extent2D { width, height };
        if self.swapchain_dirty
            || self
                .swapchain
                .as_ref()
                .is_none_or(|swapchain| swapchain.requested_extent != requested_extent)
        {
            self.recreate_swapchain(requested_extent)?;
        }
        let Some(swapchain) = &self.swapchain else {
            return Ok(false);
        };
        let vk::Extent2D { width, height } = swapchain.extent;
        let groups = [
            width.div_ceil(WORKGROUP_SIZE),
            height.div_ceil(WORKGROUP_SIZE),
        ];
        let limits = &self.device.limits;
        ensure!(
            groups[0] <= limits.max_compute_work_group_count[0]
                && groups[1] <= limits.max_compute_work_group_count[1],
            "render extent exceeds the device's compute dispatch limits"
        );
        let max_bytes = (limits.max_storage_buffer_range as usize).min(isize::MAX as usize);
        let scene_bytes = pack_scene(scene, width, height, swapchain.bgra, max_bytes)?;
        let input_info = [self.dispatch.upload(&self.device, &scene_bytes)?];
        let output_info = [swapchain.pixels.descriptor(swapchain.pixels.size)];
        let set = self.dispatch.set;
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&input_info),
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&output_info),
        ];
        unsafe { self.device.handle.update_descriptor_sets(&writes, &[]) };

        let acquired = unsafe {
            swapchain.loader.acquire_next_image(
                swapchain.handle,
                16_000_000,
                self.dispatch.acquired,
                vk::Fence::null(),
            )
        };
        let (index, suboptimal) = match acquired {
            Ok(image) => image,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain_dirty = true;
                return Ok(true);
            }
            Err(vk::Result::TIMEOUT | vk::Result::NOT_READY) => return Ok(true),
            Err(error) => return Err(error).context("acquire a swapchain image"),
        };
        let image = &swapchain.images[index as usize];
        self.record(swapchain, image, groups)?;
        self.dispatch
            .submit(self.device.compute_queue, image.ready)?;

        let waits = [image.ready];
        let chains = [swapchain.handle];
        let indices = [index];
        let present = vk::PresentInfoKHR::default()
            .wait_semaphores(&waits)
            .swapchains(&chains)
            .image_indices(&indices);
        before_present(&self.window);
        match unsafe {
            swapchain
                .loader
                .queue_present(self.device.present_queue, &present)
        } {
            Ok(present_suboptimal) => {
                self.swapchain_dirty = suboptimal || present_suboptimal;
                Ok(false)
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain_dirty = true;
                Ok(true)
            }
            Err(error) => Err(error).context("present the ray marched frame"),
        }
    }

    fn recreate_swapchain(&mut self, extent: vk::Extent2D) -> Result<()> {
        unsafe { self.device.handle.device_wait_idle() }
            .context("wait to recreate the swapchain")?;
        let old = self.swapchain.take();
        let old_handle = old
            .as_ref()
            .map_or(vk::SwapchainKHR::null(), |swapchain| swapchain.handle);
        self.swapchain = Swapchain::new(&self.device, extent, old_handle)?;
        self.swapchain_dirty = false;
        Ok(())
    }

    fn record(&self, swapchain: &Swapchain, image: &PresentImage, groups: [u32; 2]) -> Result<()> {
        let device = &self.device.handle;
        let command = self.dispatch.command;
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        let pixels_written = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
            .dst_access_mask(vk::AccessFlags::TRANSFER_READ);
        let subresource = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .level_count(1)
            .layer_count(1);
        let transition = vk::ImageMemoryBarrier::default()
            .image(image.handle)
            .subresource_range(subresource)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED);
        let to_copy = transition
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
        let to_present = transition
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE);
        let copy = vk::BufferImageCopy::default()
            .image_subresource(
                vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1),
            )
            .image_extent(vk::Extent3D {
                width: swapchain.extent.width,
                height: swapchain.extent.height,
                depth: 1,
            });
        unsafe {
            device
                .begin_command_buffer(command, &begin_info)
                .context("begin recording the ray march dispatch")?;
            device.cmd_bind_pipeline(command, vk::PipelineBindPoint::COMPUTE, self.pipeline);
            device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline_layout,
                0,
                &[self.dispatch.set],
                &[],
            );
            device.cmd_dispatch(command, groups[0], groups[1], 1);
            // The transfer stage also waits for acquisition before transitioning the image.
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[pixels_written],
                &[],
                &[to_copy],
            );
            device.cmd_copy_buffer_to_image(
                command,
                swapchain.pixels.handle,
                image.handle,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[copy],
            );
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_present],
            );
            device
                .end_command_buffer(command)
                .context("record the ray march dispatch")?;
        }
        Ok(())
    }
}

impl<W> Drop for Renderer<W> {
    fn drop(&mut self) {
        unsafe {
            let device = &self.device.handle;
            let _ = device.device_wait_idle();
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_descriptor_set_layout(self.descriptor_layout, None);
        }
    }
}

struct Dispatch {
    device: ash::Device,
    descriptor_pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    command_pool: vk::CommandPool,
    command: vk::CommandBuffer,
    acquired: vk::Semaphore,
    fence: vk::Fence,
    pending: bool,
    input: Option<StorageBuffer>,
}

impl Dispatch {
    fn new(device: &ComputeDevice) -> Result<Self> {
        let mut dispatch = Self {
            device: device.handle.clone(),
            descriptor_pool: vk::DescriptorPool::null(),
            set: vk::DescriptorSet::null(),
            command_pool: vk::CommandPool::null(),
            command: vk::CommandBuffer::null(),
            acquired: vk::Semaphore::null(),
            fence: vk::Fence::null(),
            pending: false,
            input: None,
        };
        let pool_sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(2)];
        let descriptor_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(1)
            .pool_sizes(&pool_sizes);
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(device.compute_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        unsafe {
            dispatch.descriptor_pool = dispatch
                .device
                .create_descriptor_pool(&descriptor_info, None)
                .context("create the dispatch descriptor pool")?;
            dispatch.command_pool = dispatch
                .device
                .create_command_pool(&pool_info, None)
                .context("create the dispatch command pool")?;
            let commands = vk::CommandBufferAllocateInfo::default()
                .command_pool(dispatch.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            dispatch.command = dispatch
                .device
                .allocate_command_buffers(&commands)
                .context("allocate the ray march command buffer")?[0];
            dispatch.acquired = dispatch
                .device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .context("create the image acquisition semaphore")?;
            dispatch.fence = dispatch
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .context("create the dispatch fence")?;
        }
        Ok(dispatch)
    }

    fn upload(&mut self, device: &ComputeDevice, bytes: &[u8]) -> Result<vk::DescriptorBufferInfo> {
        if self
            .input
            .as_ref()
            .is_none_or(|buffer| buffer.size < bytes.len())
        {
            self.input = Some(StorageBuffer::new(device, bytes.len(), true)?);
        }
        let input = self.input.as_mut().expect("scene buffer was allocated");
        input.upload(bytes);
        Ok(input.descriptor(bytes.len()))
    }

    fn submit(&mut self, queue: vk::Queue, ready: vk::Semaphore) -> Result<()> {
        let commands = [self.command];
        let waits = [self.acquired];
        let stages = [vk::PipelineStageFlags::TRANSFER];
        let signals = [ready];
        let submit = vk::SubmitInfo::default()
            .wait_semaphores(&waits)
            .wait_dst_stage_mask(&stages)
            .command_buffers(&commands)
            .signal_semaphores(&signals);
        unsafe {
            self.device
                .reset_fences(&[self.fence])
                .context("reset the dispatch fence")?;
            self.device
                .queue_submit(queue, &[submit], self.fence)
                .context("submit the ray march dispatch")?;
        }
        self.pending = true;
        Ok(())
    }

    fn wait(&mut self) -> Result<()> {
        if self.pending {
            unsafe { self.device.wait_for_fences(&[self.fence], true, u64::MAX) }
                .context("wait for the previous ray march dispatch")?;
            self.pending = false;
        }
        Ok(())
    }
}

impl Drop for Dispatch {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_semaphore(self.acquired, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}

fn pack_scene(
    scene: &Scene,
    width: u32,
    height: u32,
    bgra: bool,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let [x, y, z] = scene.camera.eye;
    let [tx, ty, tz] = scene.camera.target;
    let horizontal_squared = (tx - x).powi(2) + (tz - z).powi(2);
    let distance_squared = horizontal_squared + (ty - y).powi(2);
    ensure!(
        horizontal_squared >= f32::MIN_POSITIVE && distance_squared.is_finite(),
        "camera must look in a finite direction with a horizontal component"
    );
    let fov = scene.camera.vertical_fov_degrees;
    ensure!(
        fov > 0.0 && fov < 180.0,
        "vertical FOV must be between 0 and 180 degrees"
    );

    let sphere_count = scene.spheres.len();
    let triangle_count = scene.triangles.len();
    let header_size = size_of::<SceneHeader>();
    // The runtime array needs one vec4 even when both geometry lists are empty.
    let geometry_bytes = sphere_count
        .saturating_mul(size_of::<GpuSphere>())
        .saturating_add(triangle_count.saturating_mul(size_of::<GpuTriangle>()))
        .max(size_of::<[f32; 4]>());
    ensure!(
        geometry_bytes <= max_bytes.saturating_sub(header_size),
        "scene exceeds the device's storage buffer limit"
    );
    let header = SceneHeader {
        eye_and_fov_scale: [x, y, z, (0.5 * fov.to_radians()).tan()],
        look_at: [tx, ty, tz, 0.0],
        extent_format: [width, height, u32::from(bgra), 0],
        geometry_counts: [sphere_count as u32, triangle_count as u32, 0, 0],
    };
    let mut bytes = Vec::with_capacity(header_size + geometry_bytes);
    bytes.extend_from_slice(bytes_of(&header));
    for sphere in &scene.spheres {
        ensure!(
            sphere.radius.is_finite()
                && sphere.radius > 0.0
                && sphere
                    .center
                    .iter()
                    .all(|coordinate| coordinate.is_finite())
                && sphere
                    .albedo
                    .iter()
                    .all(|color| (0.0..=1.0).contains(color)),
            "spheres require a finite center, a positive finite radius, and albedo in 0..=1"
        );
        let [x, y, z] = sphere.center;
        let [r, g, b] = sphere.albedo;
        let sphere = GpuSphere {
            center_radius: [x, y, z, sphere.radius],
            albedo: [r, g, b, 0.0],
        };
        bytes.extend_from_slice(bytes_of(&sphere));
    }
    for triangle in &scene.triangles {
        ensure!(
            triangle
                .vertices
                .iter()
                .flatten()
                .all(|coordinate| coordinate.is_finite())
                && triangle
                    .albedo
                    .iter()
                    .all(|color| (0.0..=1.0).contains(color)),
            "triangles require finite vertices and albedo in 0..=1"
        );
        let [r, g, b] = triangle.albedo;
        let triangle = GpuTriangle {
            vertices: triangle.vertices.map(|[x, y, z]| [x, y, z, 0.0]),
            albedo: [r, g, b, 0.0],
        };
        bytes.extend_from_slice(bytes_of(&triangle));
    }
    bytes.resize(header_size + geometry_bytes, 0);
    Ok(bytes)
}
