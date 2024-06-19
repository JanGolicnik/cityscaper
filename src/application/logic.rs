use jandering_engine::{
    object::Instance,
    renderer::{Janderer, Renderer},
    types::{Mat4, Qua, Vec2, Vec3},
};
use rand::{rngs::ThreadRng, Rng};
use serde::Deserialize;

use crate::{
    color_obj::{AgeObject, AgeVertex},
    icosphere,
    image::Image,
    l_system::{self, config::LConfig, RenderShape},
};

use super::{cylinder, Application};

const DUST_SCALE: Vec3 = Vec3::splat(0.0085);

lazy_static::lazy_static! {
    static ref CYLINDER_DATA: (Vec<AgeVertex>, Vec<u32>) = gen_cylinder_data();
}

fn gen_cylinder_data() -> (Vec<AgeVertex>, Vec<u32>) {
    let (vertices, indices) = cylinder::generate(3);
    let vertices = vertices
        .into_iter()
        .map(AgeVertex::from)
        .collect::<Vec<AgeVertex>>();
    (vertices, indices)
}

fn cylinder(age: f32, next_age: f32, mat: Mat4, index_offset: u32) -> (Vec<AgeVertex>, Vec<u32>) {
    let (mut vertices, mut indices) = CYLINDER_DATA.clone();
    vertices.iter_mut().enumerate().for_each(|(i, e)| {
        if i < 3 {
            e.age = age;
        } else {
            e.age = next_age;
        }
        e.position = mat.mul_vec4(e.position.extend(1.0)).truncate();
    });
    indices.iter_mut().for_each(|e| *e += index_offset);
    (vertices, indices)
}

impl Application {
    pub fn update_dust(&mut self, dt: f32, renderer: &mut Renderer) {
        let camera = renderer.get_typed_bind_group(self.camera).unwrap();
        let ground_pos =
            camera_ground_intersection(camera.direction(), camera.position()).unwrap_or(Vec3::ZERO);
        let ground_pos = Vec2::new(ground_pos.x, ground_pos.z);

        let idle_rotation = Qua::from_axis_angle(Vec3::Y, 3.0 * dt);

        for dust in self.dust.instances.iter_mut() {
            let mat = dust.mat();
            let (mut scale, mut rotation, mut pos) = mat.to_scale_rotation_translation();
            let mut pos_2d = Vec2::new(pos.x, pos.z);
            if pos_2d.distance(ground_pos) > 7.0 || scale.x < 0.0 {
                let dist = self.rng.gen_range(0.0f32..7.0f32);
                let angle = self.rng.gen_range(0.0f32..360.0f32);

                let offset = Vec2::from_angle(angle.to_radians()) * dist;
                pos_2d = ground_pos + offset;
                pos.y = self.rng.gen_range(-0.5..0.0);
                scale = DUST_SCALE;

                let angle = self.rng.gen_range(0.0f32..360.0f32);
                rotation *= Qua::from_axis_angle(Vec3::Y, angle);
            }

            rotation *= idle_rotation;
            pos.x = pos_2d.x;
            pos.y += 0.1 * dt;
            pos.z = pos_2d.y;

            scale -= DUST_SCALE.x * dt * 0.2;

            let mat = Mat4::from_scale_rotation_translation(scale, rotation, pos);
            dust.set_mat(mat);
        }

        self.dust.update(renderer);
    }
}

fn camera_ground_intersection(dir: Vec3, cam_pos: Vec3) -> Option<Vec3> {
    let denom = Vec3::Y.dot(-dir);
    if denom > 1e-6 {
        let dif = -cam_pos;
        let t = dif.dot(Vec3::Y) / denom;
        Some(cam_pos - dir * t)
    } else {
        None
    }
}

pub fn place_pos_on_heightmap(
    mut pos: Vec3,
    iterations: u32,
    heightmap: &Image,
    rng: &mut ThreadRng,
) -> Vec3 {
    for _ in 0..=iterations {
        let mut highest_val = heightmap.sample(pos.x, pos.z);
        for i in -1..1 {
            for j in -1..1 {
                let this_pos = pos + Vec3::new(j as f32 * 0.01, 0.0, i as f32 * 0.01);
                let val = heightmap.sample(this_pos.x, this_pos.z);
                if val > highest_val {
                    highest_val = val;
                    pos = this_pos;
                }
            }
        }
    }
    pos + Vec3::new(
        rng.gen_range(-0.05..=0.05),
        0.0,
        rng.gen_range(-0.05..=0.05),
    )
}

pub fn create_plant(renderer: &mut Renderer, l_config: &LConfig, rng: &mut ThreadRng) -> AgeObject {
    let shapes = l_system::build(l_config, rng);
    if let Some((vertices, indices)) = shapes_to_mesh_data(shapes) {
        AgeObject::new(
            renderer,
            vertices,
            indices,
            vec![Instance::default().translate(Vec3::splat(0.0))],
        )
    } else {
        AgeObject::new(renderer, Vec::new(), Vec::new(), Vec::new())
    }
}

pub fn shapes_to_mesh_data(shapes: Vec<RenderShape>) -> Option<(Vec<AgeVertex>, Vec<u32>)> {
    let shape_to_mat = |shape: &RenderShape| match shape {
        RenderShape::Line {
            start, end, width, ..
        } => {
            let diff = *end - *start;
            let length = diff.length();
            let width = width * length * 0.01;
            Mat4::from_scale_rotation_translation(
                Vec3::new(width, length, width),
                Qua::from_rotation_arc(Vec3::Y, diff.normalize()),
                *start + diff * 0.5,
            )
        }
        RenderShape::Circle { size, pos, .. } => {
            Mat4::from_scale_rotation_translation(Vec3::splat(*size), Qua::default(), *pos)
        }
        _ => Mat4::IDENTITY,
    };

    if shapes.iter().fold(0, |acc, e| {
        if matches!(e, RenderShape::Line { .. } | RenderShape::Circle { .. }) {
            acc + 1
        } else {
            acc
        }
    }) == 0
    {
        return None;
    }

    let mut scopes: Vec<_> = vec![0];

    let mut get_first_data = |shapes: &mut std::vec::IntoIter<RenderShape>| {
        for shape in shapes {
            match shape {
                RenderShape::Line { age, last_age, .. } => {
                    let mat = shape_to_mat(&shape);
                    return cylinder(last_age, age, mat, 0);
                }
                RenderShape::Circle { age, .. } => {
                    let mat = shape_to_mat(&shape);
                    return icosphere::generate(age, mat, 0);
                }
                RenderShape::Scope => {
                    scopes.push(0);
                }
                RenderShape::ScopeEnd => {
                    scopes.pop();
                }
            }
        }
        (Vec::new(), Vec::new())
    };

    let mut shapes = shapes.into_iter();
    let (mut vertices, mut indices) = get_first_data(&mut shapes);

    *scopes.last_mut().unwrap() = vertices.len();

    for shape in shapes {
        let mat = shape_to_mat(&shape);
        let parent_verts = *scopes.last().unwrap();
        match shape {
            RenderShape::Line { age, .. } => {
                let new_vertices = &mut cylinder::extrude(
                    3,
                    parent_verts as u32,
                    vertices.len() as u32,
                    &mut indices,
                    age,
                    &mat,
                );
                vertices.append(new_vertices);
                *scopes.last_mut().unwrap() = vertices.len();
            }
            RenderShape::Circle { age, .. } => {
                let (mut new_vertices, mut new_indices) =
                    icosphere::generate(age, mat, vertices.len() as u32);
                vertices.append(&mut new_vertices);
                indices.append(&mut new_indices);
                *scopes.last_mut().unwrap() = vertices.len();
            }
            RenderShape::Scope => {
                scopes.push(parent_verts);
            }
            RenderShape::ScopeEnd => {
                scopes.pop();
            }
        };
    }

    Some((vertices, indices))
}

pub fn parse_lut(text: &str, linear: bool) -> Option<Vec<Vec3>> {
    #[derive(Deserialize)]
    struct Element {
        color: [f32; 3],
        age: u32,
    }
    let colors = serde_json::from_str::<Vec<Element>>(text)
        .unwrap()
        .into_iter()
        .map(|e| (e.age, e.color.into()))
        .collect::<Vec<(u32, Vec3)>>();
    let colors = if linear {
        l_system::colors::parse_colors_linear(&colors)
    } else {
        l_system::colors::parse_colors(&colors)
    };

    Some(colors)
}
