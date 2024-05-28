use jandering_engine::{
    core::{
        bind_group::{
            camera::free::{CameraController, FreeCameraController, MatrixCameraBindGroup},
            BindGroup,
        },
        engine::{Engine, EngineContext},
        event_handler::EventHandler,
        object::{Instance, Renderable, Vertex},
        renderer::{
            create_typed_bind_group, get_typed_bind_group, get_typed_bind_group_mut,
            BindGroupHandle, Renderer, ShaderHandle, TextureHandle,
        },
        shader::ShaderDescriptor,
        texture::{TextureDescriptor, TextureFormat},
        window::{InputState, Key, WindowEvent},
    },
    types::{Mat4, Qua, Vec2, Vec3},
    utils::load_text,
};
use rand::{rngs::ThreadRng, thread_rng, Rng};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    camera_controller::IsometricCameraController,
    color_obj::{ColorObject, ColorVertex},
    cylinder, icosphere,
    l_system::{self, builder::RenderShape, config::LConfig, LSystem},
    timer::Timer,
};

lazy_static::lazy_static! {
    #[derive(Debug)]
    pub static ref SHADER_CODE_MUTEX: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
}

pub struct Application {
    last_time: web_time::Instant,
    time: f32,
    shader: ShaderHandle,
    camera: BindGroupHandle<MatrixCameraBindGroup>,
    camera_controller: Box<dyn CameraController>,
    depth_texture: TextureHandle,

    #[allow(dead_code)]
    default_shader: ShaderHandle,

    plants: HashMap<(i32, i32), ColorObject>,
    l_config: LConfig,
    floor: ColorObject,

    dust: ColorObject,

    rng: ThreadRng,
}

const N_DUST: u32 = 30;
const DUST_SCALE: Vec3 = Vec3::splat(0.01);

const REFERENCE_DIAGONAL: f32 = 2202.0;
const ORTHO_WIDTH: f32 = 2.0;
const ORTHO_HEIGHT: f32 = ORTHO_WIDTH;
const ORTHO_NEAR: f32 = 0.003;
const ORTHO_FAR: f32 = 1000.0;

const N_PLANTS: u32 = 4;
const PLANT_SPACING: i32 = 3;

lazy_static::lazy_static! {
    static ref CYLINDER_DATA: (Vec<ColorVertex>, Vec<u32>) = gen_cylinder_data();
}

fn gen_cylinder_data() -> (Vec<ColorVertex>, Vec<u32>) {
    let (vertices, indices) = cylinder::generate(3);
    let vertices = vertices
        .into_iter()
        .map(ColorVertex::from)
        .collect::<Vec<ColorVertex>>();
    (vertices, indices)
}

fn cylinder(color: Vec3, mat: Mat4, index_offset: u32) -> (Vec<ColorVertex>, Vec<u32>) {
    let (mut vertices, mut indices) = CYLINDER_DATA.clone();
    vertices.iter_mut().for_each(|e| {
        e.color = color;
        e.position = mat.mul_vec4(e.position.extend(1.0)).truncate();
    });
    indices.iter_mut().for_each(|e| *e += index_offset);
    (vertices, indices)
}

impl Application {
    pub async fn new(engine: &mut Engine) -> Self {
        let (aspect, diagonal) = {
            let size = engine.renderer.size();
            let size = Vec2::new(size.x as f32, size.y as f32);
            (size.x / size.y, (size.x * size.x + size.y * size.y).sqrt())
        };
        let controller = IsometricCameraController {
            pan_speed: 0.002 * (diagonal / REFERENCE_DIAGONAL),
            ..Default::default()
        };
        let controller: Box<dyn CameraController> = Box::new(controller);
        let mut camera = MatrixCameraBindGroup::with_controller(controller);
        camera.make_ortho(
            (-ORTHO_WIDTH * aspect) / 2.0,
            (ORTHO_WIDTH * aspect) / 2.0,
            -ORTHO_HEIGHT / 2.0,
            ORTHO_HEIGHT / 2.0,
            ORTHO_NEAR,
            ORTHO_FAR,
        );
        *camera.position_mut() = Vec3::new(-9.5, 10.0, -9.5);
        *camera.direction_mut() = Vec3::new(1.0, -1.0, 1.0).normalize();
        let camera = create_typed_bind_group(engine.renderer.as_mut(), camera);

        let shader: ShaderHandle = engine.renderer.create_shader(
            ShaderDescriptor::default()
                .with_source(jandering_engine::core::shader::ShaderSource::Code(
                    load_text(jandering_engine::utils::FilePath::FileName(
                        "shaders/shader.wgsl",
                    ))
                    .await
                    .unwrap(),
                ))
                .with_descriptors(vec![ColorVertex::desc(), Instance::desc()])
                .with_bind_group_layouts(vec![MatrixCameraBindGroup::get_layout()])
                .with_depth(true)
                .with_backface_culling(false),
        );

        let default_shader: ShaderHandle = engine.renderer.create_shader(
            ShaderDescriptor::default()
                .with_descriptors(vec![Vertex::desc(), Instance::desc()])
                .with_bind_group_layouts(vec![MatrixCameraBindGroup::get_layout()])
                .with_depth(true)
                .with_backface_culling(false),
        );

        let depth_texture = engine.renderer.create_texture(TextureDescriptor {
            size: engine.renderer.size(),
            format: TextureFormat::Depth32F,
            ..Default::default()
        });

        let floor = ColorObject::quad(
            engine.renderer.as_mut(),
            Vec3::ZERO,
            vec![Instance::default()
                .rotate(90.0f32.to_radians(), Vec3::X)
                .set_size(Vec3::splat(100.0))],
        );

        let json = load_text(jandering_engine::utils::FilePath::FileName("lsystem.json"))
            .await
            .unwrap();
        let l_config = LConfig::from_json(json);

        let mut plants = HashMap::new();
        plants.reserve(50);

        let dust_instances = (0..N_DUST)
            .map(|_| {
                Instance::default()
                    .set_size(DUST_SCALE)
                    .translate(Vec3::splat(-1000.0))
            })
            .collect();
        let dust = ColorObject::quad(engine.renderer.as_mut(), Vec3::splat(0.3), dust_instances);

        Self {
            last_time: web_time::Instant::now(),
            time: 0.0,
            shader,
            camera,
            camera_controller: Box::<FreeCameraController>::default(),
            depth_texture,

            default_shader,

            plants,
            l_config,
            floor,

            dust,

            rng: thread_rng(),
        }
    }

    fn spawn_new_plants(&mut self, renderer: &mut dyn Renderer) {
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
                        // let (vertices, indices) = self.new_plant(&mut self.rng.clone());
                        let (vertices, indices) = self.new_plant2(&mut self.rng.clone());

                        let object = ColorObject::new(
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

    fn new_plant(&mut self, rng: &mut ThreadRng) -> (Vec<ColorVertex>, Vec<u32>) {
        let timer = Timer::now("building took: ".to_string());

        let l_system = LSystem::new(&self.l_config.rules, rng);

        let shapes = l_system.build(&self.l_config.rendering, rng);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        timer.print();

        let timer = Timer::now("meshing took: ".to_string());

        for shape in shapes {
            let (mut new_vertices, mut new_indices) =
                shape_to_mesh_data(shape, vertices.len() as u32);
            vertices.append(&mut new_vertices);
            indices.append(&mut new_indices);
        }

        timer.print();

        (vertices, indices)
    }

    fn new_plant2(&mut self, rng: &mut ThreadRng) -> (Vec<ColorVertex>, Vec<u32>) {
        let timer = Timer::now("building took: ".to_string());

        let shapes = l_system::test::build_lsystem(&self.l_config, rng);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        timer.print();

        let timer = Timer::now("meshing took: ".to_string());

        for shape in shapes {
            let (mut new_vertices, mut new_indices) =
                shape_to_mesh_data(shape, vertices.len() as u32);
            vertices.append(&mut new_vertices);
            indices.append(&mut new_indices);
        }

        timer.print();

        (vertices, indices)
    }

    fn update_dust(&mut self, dt: f32, renderer: &mut dyn Renderer) {
        let camera = get_typed_bind_group(renderer, self.camera).unwrap();
        let ground_pos =
            camera_ground_intersection(camera.direction(), camera.position()).unwrap_or(Vec3::ZERO);
        let ground_pos = Vec2::new(ground_pos.x, ground_pos.z);

        let idle_rotation = Qua::from_axis_angle(Vec3::Y, 3.0 * dt);

        for dust in self.dust.instances.iter_mut() {
            let mat = dust.mat();
            let (mut scale, mut rotation, mut pos) = mat.to_scale_rotation_translation();
            let mut pos_2d = Vec2::new(pos.x, pos.z);
            if pos_2d.distance(ground_pos) > 3.0 || scale.x < 0.0 {
                let dist = self.rng.gen_range(0.0f32..7.0f32);
                let angle = self.rng.gen_range(0.0f32..360.0f32);

                let offset = Vec2::from_angle(angle.to_radians()) * dist;
                pos_2d = ground_pos + offset;
                pos.y = -self.rng.gen_range(0.1..0.5);

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

fn shape_to_mesh_data(shape: RenderShape, vertices_len: u32) -> (Vec<ColorVertex>, Vec<u32>) {
    let (vertices, indices) = match shape {
        RenderShape::Line {
            start,
            end,
            width,
            color,
        } => {
            let diff = end - start;
            let length = diff.length();
            let width = width * length * 0.01;
            let mat = Mat4::from_scale_rotation_translation(
                Vec3::new(width, length, width),
                Qua::from_rotation_arc(Vec3::Y, diff.normalize()),
                start + diff * 0.5,
            );
            let (vertices, indices) = cylinder(color, mat, vertices_len);
            (vertices, indices)
        }
        RenderShape::Circle { size, pos, color } => {
            let mat = Mat4::from_scale_rotation_translation(Vec3::splat(size), Qua::default(), pos);
            let (vertices, indices) = icosphere::generate(color, mat, vertices_len);
            (vertices, indices)
        }
    };
    (vertices, indices)
}

impl EventHandler for Application {
    fn on_update(&mut self, context: &mut EngineContext) {
        let current_time = web_time::Instant::now();
        let dt = (current_time - self.last_time).as_secs_f32();
        self.last_time = current_time;
        self.time += dt;

        let mut guard = SHADER_CODE_MUTEX.lock().unwrap();
        if let Some(code) = guard.clone() {
            context.renderer.create_shader_at(
                ShaderDescriptor::default()
                    .with_source(jandering_engine::core::shader::ShaderSource::Code(code))
                    .with_descriptors(vec![Vertex::desc(), Instance::desc()])
                    .with_bind_group_layouts(vec![MatrixCameraBindGroup::get_layout()])
                    .with_depth(true)
                    .with_backface_culling(true),
                self.shader,
            );
            *guard = None;
        }

        if context.events.matches(|e| {
            matches!(
                e,
                WindowEvent::KeyInput {
                    key: Key::V,
                    state: InputState::Pressed
                }
            )
        }) {
            wasm_bindgen_futures::spawn_local(async move {
                let text = load_text(jandering_engine::utils::FilePath::FileName(
                    "shaders/shader.wgsl",
                ))
                .await
                .unwrap();

                let mut guard = SHADER_CODE_MUTEX.lock().unwrap();
                *guard = Some(text);
            });
        }

        if context.events.matches(|e| {
            matches!(
                e,
                WindowEvent::KeyInput {
                    key: Key::F,
                    state: InputState::Pressed
                }
            )
        }) {
            let aspect = {
                let size = context.renderer.size();
                let size = Vec2::new(size.x as f32, size.y as f32);
                size.x / size.y
            };
            let camera = get_typed_bind_group_mut(context.renderer.as_mut(), self.camera).unwrap();
            std::mem::swap(
                camera.controller.as_mut().unwrap(),
                &mut self.camera_controller,
            );
            camera.make_perspective(35.0, aspect, 0.01, 10000.0);
        }

        if context.events.matches(|e| {
            matches!(
                e,
                WindowEvent::KeyInput {
                    key: Key::G,
                    state: InputState::Pressed
                }
            )
        }) {
            let aspect = {
                let size = context.renderer.size();
                let size = Vec2::new(size.x as f32, size.y as f32);
                size.x / size.y
            };
            let camera = get_typed_bind_group_mut(context.renderer.as_mut(), self.camera).unwrap();
            std::mem::swap(
                camera.controller.as_mut().unwrap(),
                &mut self.camera_controller,
            );
            camera.make_ortho(
                (-ORTHO_WIDTH * aspect) / 2.0,
                (ORTHO_WIDTH * aspect) / 2.0,
                5.0 - ORTHO_HEIGHT / 2.0,
                ORTHO_HEIGHT / 2.0,
                ORTHO_NEAR,
                ORTHO_FAR,
            );
        }

        if context
            .events
            .matches(|e| matches!(e, WindowEvent::Resized(_)))
        {
            let aspect = {
                let size = context.renderer.size();
                size.x as f32 / size.y as f32
            };
            let camera = get_typed_bind_group_mut(context.renderer.as_mut(), self.camera).unwrap();
            camera.make_ortho(
                (-ORTHO_WIDTH * aspect) / 2.0,
                (ORTHO_WIDTH * aspect) / 2.0,
                -ORTHO_HEIGHT / 2.0,
                ORTHO_HEIGHT / 2.0,
                ORTHO_NEAR,
                ORTHO_FAR,
            );

            context.renderer.re_create_texture(
                TextureDescriptor {
                    size: context.renderer.size(),
                    format: TextureFormat::Depth32F,
                    ..Default::default()
                },
                self.depth_texture,
            );
        }

        let camera = get_typed_bind_group_mut(context.renderer.as_mut(), self.camera).unwrap();
        camera.update(context.events, dt);

        self.spawn_new_plants(context.renderer.as_mut());
        self.update_dust(dt, context.renderer.as_mut());
    }

    fn on_render(&mut self, renderer: &mut Box<dyn Renderer>) {
        let camera = get_typed_bind_group(renderer.as_ref(), self.camera).unwrap();
        renderer.write_bind_group(self.camera.into(), &camera.get_data());

        let plants = self
            .plants
            .values()
            .map(|e| e as &dyn Renderable)
            .collect::<Vec<_>>();

        renderer
            .new_pass()
            .with_depth(self.depth_texture, Some(1.0))
            .with_clear_color(0.2, 0.5, 1.0)
            .set_shader(self.shader)
            .bind(0, self.camera.into())
            .render(&plants)
            .render(&[&self.floor, &self.dust])
            .submit();
    }
}
