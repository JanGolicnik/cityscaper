use jandering_engine::types::Mat4;

use crate::color_obj::AgeVertex;

type Triangle = [u32; 3];

mod icosahedron {
    use jandering_engine::types::Vec3;

    use super::Triangle;

    pub(crate) const X: f32 = 0.525_731_1;
    pub(crate) const Z: f32 = 0.850_650_8;
    pub(crate) const N: f32 = 0.0;

    pub(crate) const VERTICES: &[Vec3] = &[
        Vec3::new(-X, N, Z),
        Vec3::new(X, N, Z),
        Vec3::new(-X, N, -Z),
        Vec3::new(X, N, -Z),
        Vec3::new(N, Z, X),
        Vec3::new(N, Z, -X),
        Vec3::new(N, -Z, X),
        Vec3::new(N, -Z, -X),
        Vec3::new(Z, X, N),
        Vec3::new(-Z, X, N),
        Vec3::new(Z, -X, N),
        Vec3::new(-Z, -X, N),
    ];

    pub(crate) const TRIANGLES: &[Triangle] = &[
        [0, 4, 1],
        [0, 9, 4],
        [9, 5, 4],
        [4, 5, 8],
        [4, 8, 1],
        [8, 10, 1],
        [8, 3, 10],
        [5, 3, 8],
        [5, 2, 3],
        [2, 7, 3],
        [7, 10, 3],
        [7, 6, 10],
        [7, 11, 6],
        [11, 0, 6],
        [0, 1, 6],
        [6, 1, 10],
        [9, 0, 11],
        [9, 11, 2],
        [9, 2, 5],
        [7, 2, 11],
    ];
}

pub fn generate(age: f32, mat: Mat4, index_offset: u32) -> (Vec<AgeVertex>, Vec<u32>) {
    let vertices = icosahedron::VERTICES
        .iter()
        .map(|v| AgeVertex {
            position: mat.transform_vector3(*v),
            normal: v.normalize(),
            age,
            ..Default::default()
        })
        .collect();
    let indices = icosahedron::TRIANGLES
        .iter()
        .flatten()
        .map(|e| *e + index_offset)
        .collect();
    (vertices, indices)
}
