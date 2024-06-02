use jandering_engine::{
    core::{
        object::Instance,
        renderer::{get_typed_bind_group, Renderer},
    },
    types::{Mat4, Qua, Vec2, Vec3},
};
use rand::{rngs::ThreadRng, Rng};
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::{
    color_obj::{AgeObject, AgeVertex},
    icosphere,
    image::Image,
    l_system::{self, RenderShape},
    timer::Timer,
};

use super::{cylinder, Application};

const DUST_SCALE: Vec3 = Vec3::splat(0.0085);

const N_PLANTS: u32 = 4;
const PLANT_SPACING: i32 = 3;

const GRASS_RANGE: f32 = 2.75;
const GRASS_ITERATIONS: u32 = 12;
const GRASS_HEIGHT: f32 = 0.1;
const GRASS_WIDTH: f32 = 0.0075;

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
        if i % 2 == 0 {
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
    pub fn spawn_new_plants(&mut self, renderer: &mut dyn Renderer) {
        let camera = get_typed_bind_group(renderer, self.camera).unwrap();
        if let Some(ground_pos) = camera_ground_intersection(camera.direction(), camera.position())
        {
            let snapped_cam = (ground_pos / PLANT_SPACING as f32).round() * PLANT_SPACING as f32;

            let half = N_PLANTS as i32 / 2;
            self.plants.retain(|_, obj| {
                let half = (half * PLANT_SPACING) as f32;
                let pos = obj.instances.first().unwrap().position();
                (pos.x - snapped_cam.x).abs() <= half && (pos.z - snapped_cam.z).abs() <= half
            });

            for x in -half..half {
                for z in -half..half {
                    let pos = (
                        snapped_cam.x as i32 + x * PLANT_SPACING,
                        snapped_cam.z as i32 + z * PLANT_SPACING,
                    );

                    #[allow(clippy::map_entry)]
                    if !self.plants.contains_key(&pos) {
                        let (vertices, indices) = self.new_plant(&mut self.rng.clone());

                        let object = AgeObject::new(
                            renderer,
                            vertices,
                            indices,
                            vec![Instance::default().translate(Vec3::new(
                                pos.0 as f32,
                                0.0,
                                pos.1 as f32,
                            ))],
                        );
                        self.plants.insert(pos, object);
                    }
                }
            }
        }
    }

    pub fn new_plant(&mut self, rng: &mut ThreadRng) -> (Vec<AgeVertex>, Vec<u32>) {
        let timer = Timer::now("building took: ".to_string());

        let shapes = l_system::build(&self.l_config, rng);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        timer.print();

        let timer = Timer::now("meshing took: ".to_string());

        let age_change = 1.0 / self.l_config.rules.iterations as f32;
        for shape in shapes {
            let (mut new_vertices, mut new_indices) =
                shape_to_mesh_data(shape, vertices.len() as u32, age_change);
            vertices.append(&mut new_vertices);
            indices.append(&mut new_indices);
        }

        timer.print();

        (vertices, indices)
    }

    pub fn update_dust(&mut self, dt: f32, renderer: &mut dyn Renderer) {
        let camera = get_typed_bind_group(renderer, self.camera).unwrap();
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

    pub fn update_grass(&mut self, renderer: &mut dyn Renderer) {
        let camera = get_typed_bind_group(renderer, self.camera).unwrap();
        let ground_pos =
            camera_ground_intersection(camera.direction(), camera.position()).unwrap_or(Vec3::ZERO);
        let ground_pos = Vec2::new(ground_pos.x, ground_pos.z);

        for grass in self.grass.instances.iter_mut() {
            let mat = grass.mat();
            let (_, rotation, mut pos) = mat.to_scale_rotation_translation();
            let mut pos_2d = Vec2::new(pos.x, pos.z);
            if pos_2d.distance(ground_pos) > GRASS_RANGE {
                let dist = self.rng.gen_range(0.9f32..1.0f32);
                let angle = self.rng.gen_range(0.0f32..360.0f32);

                let offset = Vec2::from_angle(angle.to_radians()) * dist * GRASS_RANGE;
                pos_2d = ground_pos + offset;

                let scale_mod = 0.7 + self.noise_image.sample(pos_2d.x, pos_2d.y) * 0.6;
                let mut scale = Vec3::new(GRASS_WIDTH, GRASS_HEIGHT, 1.0) * scale_mod;
                pos.x = pos_2d.x;
                pos.z = pos_2d.y;

                pos = Self::place_pos_on_heightmap(
                    pos,
                    GRASS_ITERATIONS,
                    &self.noise_image,
                    &mut self.rng,
                );
                pos.y = 0.0;
                if (Vec3::ZERO).distance(pos) < 3.0 {
                    scale *= 0.01;
                }

                let mat = Mat4::from_scale_rotation_translation(scale, rotation, pos);
                grass.set_mat(mat);
            }
        }

        self.grass.update(renderer);
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

    pub fn update_iteration_count(&mut self) {
        if let Some(value) = web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.get_element_by_id("detail"))
            .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
            .map(|el| el.value())
            .and_then(|value| value.parse::<u32>().ok())
        {
            if self.l_config.rules.iterations != value {
                self.plants.clear();
                self.l_config.rules.iterations = value;
            }
        }
    }
}

pub fn read_lut(linear: bool) -> Option<Vec<Vec3>> {
    let elements = web_sys::window()?
        .document()?
        .get_elements_by_class_name("color-stop");
    let mut colors = Vec::with_capacity(elements.length() as usize);
    for i in 0..elements.length() {
        let element = elements.get_with_index(i)?;
        if element.id() == "color-stop-template" {
            continue;
        }
        let color = element
            .children()
            .get_with_index(1)?
            .dyn_into::<HtmlInputElement>()
            .unwrap();
        let age = element
            .last_element_child()?
            .dyn_into::<HtmlInputElement>()
            .unwrap();
        let age = age.value().parse::<u32>().unwrap_or(0);
        let color = hex_color::HexColor::parse(&color.value())
            .map(|e| Vec3::new(e.r as f32 / 255.0, e.g as f32 / 255.0, e.b as f32 / 255.0))
            .unwrap_or(Vec3::ZERO);
        colors.push((age, color));
    }

    let colors = if linear {
        l_system::colors::parse_colors_linear(&colors)
    } else {
        l_system::colors::parse_colors(&colors)
    };

    Some(colors)
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

fn shape_to_mesh_data(
    shape: RenderShape,
    vertices_len: u32,
    age_change: f32,
) -> (Vec<AgeVertex>, Vec<u32>) {
    let (vertices, indices) = match shape {
        RenderShape::Line {
            start,
            end,
            width,
            age,
        } => {
            let diff = end - start;
            let length = diff.length();
            let width = width * length * 0.01;
            let mat = Mat4::from_scale_rotation_translation(
                Vec3::new(width, length, width),
                Qua::from_rotation_arc(Vec3::Y, diff.normalize()),
                start + diff * 0.5,
            );
            let (vertices, indices) = cylinder(age, age + age_change, mat, vertices_len);
            (vertices, indices)
        }
        RenderShape::Circle { size, pos, age } => {
            let mat = Mat4::from_scale_rotation_translation(Vec3::splat(size), Qua::default(), pos);
            let (vertices, indices) = icosphere::generate(age, mat, vertices_len);
            (vertices, indices)
        }
    };
    (vertices, indices)
}
