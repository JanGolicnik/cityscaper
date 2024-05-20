use jandering_engine::{
    core::{
        bind_group::{
            camera::free::{CameraController, MatrixCameraBindGroup},
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
use serde::{de::VariantAccess, Deserialize};
use std::sync::{Arc, Mutex};

use crate::{
    camera_controller::IsometricCameraController,
    color_obj::{ColorObject, ColorVertex},
    cylinder, icosphere,
    l_system::{
        builder::{RenderConfig, RenderShape},
        LSystem, LSystemConfig,
    },
    mesh_renderer::{Mesh, MeshRenderer},
};

lazy_static::lazy_static! {
    #[derive(Debug)]
    pub static ref SHADER_CODE_MUTEX: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
}

struct Plant {
    variations: [ColorObject; NUM_PLANT_VARIATIONS],
}

pub struct Application {
    last_time: web_time::Instant,
    time: f32,
    shader: ShaderHandle,
    camera: BindGroupHandle<MatrixCameraBindGroup>,
    depth_texture: TextureHandle,
    mesh_renderer: MeshRenderer,
    intersection_instances: Vec<Instance>,
    road_instances: Vec<Instance>,

    plants: Vec<Plant>,
}

const REFERENCE_DIAGONAL: f32 = 2202.0;
const ORTHO_WIDTH: f32 = 2.0;
const ORTHO_HEIGHT: f32 = ORTHO_WIDTH;
const ORTHO_NEAR: f32 = 0.003;
const ORTHO_FAR: f32 = 1000.0;

const NUM_PLANT_VARIATIONS: usize = 5;

lazy_static::lazy_static! {
    static ref CYLINDER_DATA: (Vec<Vertex>, Vec<u32>) = cylinder::generate(3);
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
        // let controller = FreeCameraController::default();
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
        // camera.make_perspective(35.0, aspect, 0.01, 10000.0);
        // *camera.position() = Vec3::new(-30.0, 15.0, -0.0);
        // *camera.direction() = Vec3::new(1.0, 0.0, 0.0).normalize();
        *camera.position() = Vec3::new(-10.0, 10.0, -10.0);
        *camera.direction() = Vec3::new(1.0, -1.0, 1.0).normalize();
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

        let depth_texture = engine.renderer.create_texture(TextureDescriptor {
            size: engine.renderer.size(),
            format: TextureFormat::Depth32F,
            ..Default::default()
        });

        let mesh_renderer = MeshRenderer::new(engine.renderer.as_mut()).await;

        let intersection_instances = generate_intersections();
        let road_instances = generate_roads();

        let plants = make_plants(engine.renderer.as_mut()).await;

        Self {
            last_time: web_time::Instant::now(),
            time: 0.0,
            shader,
            camera,
            depth_texture,
            mesh_renderer,
            intersection_instances,
            road_instances,

            plants,
        }
    }
}

async fn make_plants(renderer: &mut dyn Renderer) -> Vec<Plant> {
    #[derive(Deserialize)]
    struct Config {
        rendering: RenderConfig,
        rules: LSystemConfig,
    }

    let json = load_text(jandering_engine::utils::FilePath::FileName("lsystem.json"))
        .await
        .unwrap();
    let (l_system, mut render_config) = match serde_json::from_str::<Config>(&json) {
        Ok(config) => {
            let l_system = LSystem::new(config.rules);
            (l_system, config.rendering)
        }
        Err(e) => panic!("{}", e.to_string()),
    };

    let variations: [ColorObject; NUM_PLANT_VARIATIONS] = (0..NUM_PLANT_VARIATIONS)
        .map(|_| {
            let shapes = l_system.build(&render_config);
            let mut vertices = Vec::new();
            let mut indices = Vec::new();

            for shape in shapes {
                let (mut new_vertices, mut new_indices) =
                    shape_to_mesh_data(shape, vertices.len() as u32);
                vertices.append(&mut new_vertices);
                indices.append(&mut new_indices);
            }

            ColorObject::new(renderer, vertices, indices, vec![Instance::default()])
        })
        .collect::<Vec<ColorObject>>()
        .try_into()
        .unwrap();

    vec![Plant { variations }]
}

fn shape_to_mesh_data(shape: RenderShape, vertices_len: u32) -> (Vec<ColorVertex>, Vec<u32>) {
    let (vertices, mut indices, mat, color) = match shape {
        RenderShape::Line {
            start,
            end,
            width,
            color,
        } => {
            let (vertices, indices) = CYLINDER_DATA.clone();
            let diff = end - start;
            let length = diff.length();
            let width = width * length * 0.01;
            let mat = Mat4::from_scale_rotation_translation(
                Vec3::new(width, length, width),
                Qua::from_rotation_arc(Vec3::Y, diff.normalize()),
                start + diff * 0.5,
            );
            (vertices, indices, mat, color)
        }
        RenderShape::Circle { size, pos, color } => {
            let (vertices, indices) = icosphere::generate(0);
            let mat = Mat4::from_scale_rotation_translation(Vec3::splat(size), Qua::default(), pos);
            (vertices, indices, mat, color)
        }
    };

    let vertices = vertices
        .into_iter()
        .map(|v| {
            let mut v = ColorVertex::from(v);
            v.position = mat.mul_vec4(v.position.extend(1.0)).truncate();
            v.color = color;
            v
        })
        .collect::<Vec<ColorVertex>>();
    indices.iter_mut().for_each(|i| *i += vertices_len);
    (vertices, indices)
}

const N_INTERSECTIONS: i32 = 10;
const ROAD_LEN: u32 = 5;
const INTERSECTION_SPACING: f32 = ROAD_LEN as f32 + 1.0;
fn generate_intersections() -> Vec<Instance> {
    (-N_INTERSECTIONS..N_INTERSECTIONS + 1)
        .flat_map(|x| {
            (-N_INTERSECTIONS..N_INTERSECTIONS + 1)
                .map(|y| {
                    Instance::default()
                        .translate(Vec3::new(x as f32, 0.0, y as f32) * INTERSECTION_SPACING)
                })
                .collect::<Vec<Instance>>()
        })
        .collect()
}

fn generate_roads() -> Vec<Instance> {
    let mut instances = Vec::new();
    let min_z = -N_INTERSECTIONS as f32 * INTERSECTION_SPACING;
    (-N_INTERSECTIONS..N_INTERSECTIONS).for_each(|x| {
        let x = x as f32 * INTERSECTION_SPACING + 1.0;
        (0..N_INTERSECTIONS * 2 + 1).for_each(|z| {
            (0..ROAD_LEN).for_each(|i| {
                let pos = Vec3::new(x + i as f32, 0.0, min_z + z as f32 * INTERSECTION_SPACING);
                instances.push(Instance::default().translate(pos));
            })
        });
    });

    let min_x = -N_INTERSECTIONS as f32 * INTERSECTION_SPACING;
    (-N_INTERSECTIONS..N_INTERSECTIONS).for_each(|z| {
        let z = z as f32 * INTERSECTION_SPACING + 1.0;
        (0..N_INTERSECTIONS * 2 + 1).for_each(|x| {
            (0..ROAD_LEN).for_each(|i| {
                let pos = Vec3::new(min_x + x as f32 * INTERSECTION_SPACING, 0.0, z + i as f32);
                instances.push(
                    Instance::default()
                        .rotate(std::f32::consts::PI * 0.5, Vec3::Y)
                        .translate(pos),
                );
            })
        });
    });

    instances
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

        if context.events.iter().any(|e| {
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

        if context
            .events
            .iter()
            .any(|e| matches!(e, WindowEvent::Resized(_)))
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

        self.mesh_renderer
            .render_mesh(Mesh::Intersection, self.intersection_instances.clone());
        self.mesh_renderer
            .render_mesh(Mesh::Road, self.road_instances.clone());
        self.mesh_renderer.update(context.renderer.as_mut());
    }

    fn on_render(&mut self, renderer: &mut Box<dyn Renderer>) {
        let camera = get_typed_bind_group(renderer.as_ref(), self.camera).unwrap();
        renderer.write_bind_group(self.camera.into(), &camera.get_data());

        let plants = self
            .plants
            .iter()
            .flat_map(|e| &e.variations)
            .map(|e| e as &dyn Renderable)
            .collect::<Vec<_>>();

        let render_pass = renderer
            .new_pass()
            .with_depth(self.depth_texture, Some(1.0))
            .with_clear_color(0.2, 0.5, 1.0)
            .set_shader(self.shader)
            .bind(0, self.camera.into());
        self.mesh_renderer
            .bind_meshes(render_pass)
            .render(&plants)
            .submit();
    }
}
