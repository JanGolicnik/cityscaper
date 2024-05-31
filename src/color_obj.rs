use jandering_engine::{
    core::{
        object::{
            primitives::{quad_data, triangle_data},
            Instance, ObjectRenderData, Renderable, Vertex,
        },
        renderer::{BufferHandle, Renderer},
        shader::{
            BufferLayout, BufferLayoutEntry, BufferLayoutEntryDataType, BufferLayoutStepMode,
        },
    },
    types::Vec3,
};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug, Default)]
pub struct ColorVertex {
    pub position: Vec3,
    pub position_padding: f32,
    pub normal: Vec3,
    pub normal_padding: f32,
    pub color: Vec3,
    pub color_padding: f32,
}

impl ColorVertex {
    pub fn desc() -> BufferLayout {
        BufferLayout {
            step_mode: BufferLayoutStepMode::Vertex,
            entries: &[
                BufferLayoutEntry {
                    location: 0,
                    data_type: BufferLayoutEntryDataType::Float32x4,
                },
                BufferLayoutEntry {
                    location: 1,
                    data_type: BufferLayoutEntryDataType::Float32x4,
                },
                BufferLayoutEntry {
                    location: 2,
                    data_type: BufferLayoutEntryDataType::Float32x4,
                },
            ],
        }
    }
}

#[derive(Debug)]
pub struct ColorObject {
    pub vertices: Vec<ColorVertex>,
    //
    pub indices: Vec<u32>,
    //
    pub instances: Vec<Instance>,
    //
    pub render_data: ObjectRenderData,

    previous_instances_len: usize,
}

impl ColorObject {
    pub fn new(
        renderer: &mut dyn Renderer,
        vertices: Vec<ColorVertex>,
        indices: Vec<u32>,
        instances: Vec<Instance>,
    ) -> Self {
        let render_data = {
            let vertex_buffer = renderer.create_vertex_buffer(bytemuck::cast_slice(&vertices));
            let instance_buffer = renderer.create_vertex_buffer(bytemuck::cast_slice(&instances));
            let index_buffer = renderer.create_index_buffer(bytemuck::cast_slice(&indices));
            ObjectRenderData {
                vertex_buffer,
                instance_buffer,
                index_buffer,
            }
        };

        let previous_instances_len = instances.len();

        Self {
            vertices,
            indices,
            instances,
            render_data,
            previous_instances_len,
        }
    }
    #[allow(dead_code)]
    pub fn update(&mut self, renderer: &mut dyn Renderer) {
        if self.previous_instances_len != self.instances.len() {
            self.render_data.instance_buffer =
                renderer.create_vertex_buffer(bytemuck::cast_slice(&self.instances));
            self.previous_instances_len = self.instances.len();
        } else {
            renderer.write_buffer(
                self.render_data.instance_buffer,
                bytemuck::cast_slice(&self.instances),
            );
        }
    }

    pub fn quad(renderer: &mut dyn Renderer, color: Vec3, instances: Vec<Instance>) -> Self {
        let (vertices, indices) = quad_data();
        let vertices = vertices
            .into_iter()
            .map(|e| {
                let mut v = ColorVertex::from(e);
                v.color = color;
                v
            })
            .collect();

        Self::new(renderer, vertices, indices, instances)
    }

    pub fn triangle(renderer: &mut dyn Renderer, color: Vec3, instances: Vec<Instance>) -> Self {
        let (vertices, indices) = triangle_data();
        let vertices = vertices
            .into_iter()
            .map(|e| {
                let mut v = ColorVertex::from(e);
                v.color = color;
                v
            })
            .collect();

        Self::new(renderer, vertices, indices, instances)
    }
}

impl Renderable for ColorObject {
    fn num_instances(&self) -> u32 {
        self.previous_instances_len as u32
    }

    fn num_indices(&self) -> u32 {
        self.indices.len() as u32
    }

    fn get_buffers(&self) -> (BufferHandle, BufferHandle, Option<BufferHandle>) {
        (
            self.render_data.vertex_buffer,
            self.render_data.index_buffer,
            Some(self.render_data.instance_buffer),
        )
    }
}

impl From<Vertex> for ColorVertex {
    fn from(v: Vertex) -> Self {
        ColorVertex {
            position: v.position,
            normal: v.normal,
            ..Default::default()
        }
    }
}
