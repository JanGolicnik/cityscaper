use jandering_engine::{
    object::Vertex,
    types::{Mat4, Vec3},
};

use crate::color_obj::AgeVertex;

pub fn generate(resolution: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();

    let mut top_vertices = Vec::new();
    (0..resolution).for_each(|i| {
        let ratio = i as f32 / resolution as f32;
        let r = ratio * std::f32::consts::PI * 2.0;
        let x = r.cos();
        let z = r.sin();
        let normal = Vec3::new(x, 0.0, z).normalize();
        vertices.push(Vertex {
            position: Vec3::new(x, -0.5, z),
            normal,
            ..Default::default()
        });
        top_vertices.push(Vertex {
            position: Vec3::new(x, 0.5, z),
            normal,
            ..Default::default()
        });
    });
    vertices.append(&mut top_vertices);

    let mut indices = Vec::new();
    (0..resolution).for_each(|i| {
        let j = resolution + i;
        let k = (i + 2) % resolution;
        let l = resolution + (i + 1) % resolution;
        indices.push(i);
        indices.push(j);
        indices.push(k);

        indices.push(j);
        indices.push(k);
        indices.push(l);
    });

    (vertices, indices)
}

pub fn extrude(
    resolution: u32,
    mut parent_verts: u32,
    vertices_len: u32,
    indices: &mut Vec<u32>,
    age: f32,
    mat: &Mat4,
) -> Vec<AgeVertex> {
    let mut vertices = Vec::new();

    (0..resolution).for_each(|i| {
        let ratio = i as f32 / resolution as f32;
        let r = ratio * std::f32::consts::PI * 2.0;
        let x = r.cos();
        let z = r.sin();
        let normal = Vec3::new(x, 0.0, z).normalize();
        let position = mat.transform_point3(Vec3::new(x, 0.5, z));
        vertices.push(AgeVertex {
            position,
            normal,
            age,
            ..Default::default()
        });
    });

    parent_verts = parent_verts.saturating_sub(resolution);
    (0..resolution).for_each(|i| {
        let j = parent_verts + i;
        let k = vertices_len + i;
        let l = parent_verts + ((i + 1) % resolution);
        let m = vertices_len + (i + 1) % (resolution);
        indices.push(j);
        indices.push(k);
        indices.push(l);

        indices.push(k);
        indices.push(l);
        indices.push(m);

        if j >= vertices_len + resolution
            || k >= vertices_len + resolution
            || l >= vertices_len + resolution
            || m >= vertices_len + resolution
        {
            panic!();
        }
    });

    vertices
}
