use std::io::Cursor;

use anyhow::{Context, Result, ensure};
use ash::vk;

const SHADER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/raymarch.spv"));
// Matches @workgroup_size in raymarch.wgsl.
const WORKGROUP_SIZE: u32 = 8;

pub(super) struct Pipeline {
    device: ash::Device,
    pub descriptor_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    handle: vk::Pipeline,
}

impl Pipeline {
    pub(super) fn new(device: &ash::Device) -> Result<Self> {
        let mut pipeline = Self {
            device: device.clone(),
            descriptor_layout: create_descriptor_layout(device)?,
            layout: vk::PipelineLayout::null(),
            handle: vk::Pipeline::null(),
        };
        let layouts = [pipeline.descriptor_layout];
        let info = vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts);
        pipeline.layout = unsafe { device.create_pipeline_layout(&info, None) }
            .context("create the ray march pipeline layout")?;
        pipeline.handle = create_compute_pipeline(device, pipeline.layout)?;
        Ok(pipeline)
    }

    pub(super) fn record(
        &self,
        command: vk::CommandBuffer,
        set: vk::DescriptorSet,
        [x, y]: [u32; 2],
    ) {
        let bind_point = vk::PipelineBindPoint::COMPUTE;
        unsafe {
            self.device
                .cmd_bind_pipeline(command, bind_point, self.handle);
            self.device
                .cmd_bind_descriptor_sets(command, bind_point, self.layout, 0, &[set], &[]);
            self.device.cmd_dispatch(command, x, y, 1);
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.handle, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_layout, None);
        }
    }
}

fn create_descriptor_layout(device: &ash::Device) -> Result<vk::DescriptorSetLayout> {
    let bindings = [0, 1].map(|binding| {
        vk::DescriptorSetLayoutBinding::default()
            .binding(binding)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
    });
    let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
    unsafe { device.create_descriptor_set_layout(&info, None) }
        .context("create the ray march descriptor layout")
}

fn create_compute_pipeline(
    device: &ash::Device,
    layout: vk::PipelineLayout,
) -> Result<vk::Pipeline> {
    let spirv = ash::util::read_spv(&mut Cursor::new(SHADER)).context("read ray march SPIR-V")?;
    let shader_info = vk::ShaderModuleCreateInfo::default().code(&spirv);
    let shader = unsafe { device.create_shader_module(&shader_info, None) }
        .context("create the ray march shader module")?;
    let stage = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::COMPUTE)
        .module(shader)
        .name(c"main");
    let infos = [vk::ComputePipelineCreateInfo::default()
        .stage(stage)
        .layout(layout)];
    let result = unsafe {
        let result = device.create_compute_pipelines(vk::PipelineCache::null(), &infos, None);
        device.destroy_shader_module(shader, None);
        result
    };
    match result {
        Ok(pipelines) => Ok(pipelines[0]),
        Err((pipelines, error)) => {
            for pipeline in pipelines {
                unsafe { device.destroy_pipeline(pipeline, None) };
            }
            Err(error).context("create the ray march compute pipeline")
        }
    }
}

pub(super) fn dispatch_groups(extent: vk::Extent2D, limits: [u32; 3]) -> Result<[u32; 2]> {
    let groups = [
        extent.width.div_ceil(WORKGROUP_SIZE),
        extent.height.div_ceil(WORKGROUP_SIZE),
    ];
    ensure!(
        groups[0] <= limits[0] && groups[1] <= limits[1],
        "render extent exceeds the device's compute dispatch limits"
    );
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_rounds_up_and_checks_both_axis_limits() {
        let extent = vk::Extent2D {
            width: 9,
            height: 17,
        };
        assert_eq!(dispatch_groups(extent, [2, 3, 1]).unwrap(), [2, 3]);
        assert!(dispatch_groups(extent, [1, 3, 1]).is_err());
        assert!(dispatch_groups(extent, [2, 2, 1]).is_err());
    }
}
