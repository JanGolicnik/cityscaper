use jandering_engine::{
    core::{
        object::{Instance, ObjectRenderData, Renderable},
        renderer::{BufferHandle, Renderer},
    },
    types::{UVec2, Vec2, Vec3},
};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct ColorVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
}

impl ColorVertex {}

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

pub fn load_obj_color(data: &str) -> (Vec<ColorVertex>, Vec<u32>) {
    let mut positions_and_colors = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut groups: Vec<Vec<u32>> = Vec::new();

    for line in data
        .lines()
        .filter(|e| !matches!(e.chars().next(), Some('#') | None))
    {
        let seprated = line.split(' ').collect::<Vec<_>>();
        match seprated.first() {
            Some(&"v") => {
                let x = seprated[1].parse::<f32>().unwrap();
                let y = seprated[2].parse::<f32>().unwrap();
                let z = seprated[3].parse::<f32>().unwrap();

                let r = seprated[4].parse::<f32>().unwrap();
                let g = seprated[5].parse::<f32>().unwrap();
                let b = seprated[6].parse::<f32>().unwrap();

                positions_and_colors.push((Vec3::new(x, y, z), Vec3::new(r, g, b)));
            }
            Some(&"vn") => {
                let x = seprated[1].parse::<f32>().unwrap();
                let y = seprated[2].parse::<f32>().unwrap();
                let z = seprated[3].parse::<f32>().unwrap();
                normals.push(Vec3::new(x, y, z));
            }
            Some(&"vt") => {
                let x = seprated[1].parse::<f32>().unwrap();
                let y = seprated[2].parse::<f32>().unwrap();
                uvs.push(Vec2::new(x, y));
            }
            Some(&"f") => {
                let mut arr: Vec<Vec<u32>> = (1..4)
                    .map(|i| {
                        seprated[i]
                            .split('/')
                            .map(|e| e.parse::<u32>().unwrap().saturating_sub(1))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                groups.append(&mut arr);
            }
            _ => {}
        }
    }

    let mut indices = Vec::new();
    let mut vertices = Vec::new();
    let mut mapped_vertices: Vec<(UVec2, u32)> = Vec::new();
    for group in groups {
        let key = UVec2::new(group[0], group[1]);
        if let Some(e) = mapped_vertices.iter().find(|e| e.0 == key) {
            // TODO: optimize this
            indices.push(e.1)
        } else {
            let index = vertices.len() as u32;
            indices.push(index);
            vertices.push(ColorVertex {
                position: positions_and_colors[group[0] as usize].0,
                normal: normals[group[2] as usize],
                color: positions_and_colors[group[0] as usize].1,
            });
            mapped_vertices.push((key, index))
        }
    }

    (vertices, indices)
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

    pub fn from_obj(data: &str, renderer: &mut dyn Renderer, instances: Vec<Instance>) -> Self {
        let (vertices, indices) = load_obj_color(data);
        Self::new(renderer, vertices, indices, instances)
    }

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
