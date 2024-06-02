use jandering_engine::core::{
    bind_group::{BindGroup, BindGroupLayout, BindGroupLayoutEntry},
    renderer::{BufferHandle, Renderer},
};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]

pub struct RenderDataData {
    pub time: f32,
    pub wind_strength: f32,
    pub wind_scale: f32,
    pub wind_speed: f32,
    pub wind_direction: f32,
    pub wind_noise_scale: f32,
    pub wind_noise_strength: f32,
    padding: [f32; 1],
}

pub struct RenderDataBindGroup {
    pub data: RenderDataData,

    buffer_handle: BufferHandle,
}

impl BindGroup for RenderDataBindGroup {
    fn get_data(&self) -> Box<[u8]> {
        bytemuck::cast_slice(&[self.data]).into()
    }

    fn get_layout(&self, _renderer: &mut dyn Renderer) -> BindGroupLayout {
        BindGroupLayout {
            entries: vec![BindGroupLayoutEntry::Data(self.buffer_handle)],
        }
    }
}

impl RenderDataBindGroup {
    pub fn new(renderer: &mut dyn Renderer) -> Self {
        let data = RenderDataData {
            time: 0.0,
            wind_strength: 0.21,
            wind_scale: 1.0,
            wind_speed: 5.0,
            wind_direction: 0.0,
            wind_noise_scale: 0.05,
            wind_noise_strength: 5.0,
            padding: [0.0; 1],
        };

        let buffer_handle = renderer.create_uniform_buffer(bytemuck::cast_slice(&[data]));

        Self {
            data,
            buffer_handle,
        }
    }

    pub fn get_layout() -> BindGroupLayout {
        BindGroupLayout {
            entries: vec![BindGroupLayoutEntry::Data(BufferHandle(0))],
        }
    }
}
