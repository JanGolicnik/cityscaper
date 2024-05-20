use jandering_engine::{core::object::Vertex, types::Vec3};

pub fn generate(resolution: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();

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
        vertices.push(Vertex {
            position: Vec3::new(x, 0.5, z),
            normal,
            ..Default::default()
        });
    });

    let mut indices = Vec::new();
    let resolution_2 = resolution * 2;
    (0..resolution).for_each(|mut i| {
        i *= 2;
        let j = i + 1;
        let k = (i + 2) % resolution_2;
        let l = (i + 3) % resolution_2;
        indices.push(i);
        indices.push(j);
        indices.push(k);

        indices.push(j);
        indices.push(k);
        indices.push(l);
    });

    (vertices, indices)
}
