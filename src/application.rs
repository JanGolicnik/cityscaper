use std::sync::{Arc, Mutex};

use jandering_engine::{
    core::{
        bind_group::{
            camera::free::{CameraController, MatrixCameraBindGroup},
            BindGroup,
        },
        engine::{Engine, EngineContext},
        event_handler::EventHandler,
        object::{Instance, Object, Vertex},
        renderer::{
            create_typed_bind_group, get_typed_bind_group, get_typed_bind_group_mut,
            BindGroupHandle, Renderer, ShaderHandle, TextureHandle,
        },
        shader::ShaderDescriptor,
        texture::{TextureDescriptor, TextureFormat},
        window::{InputState, Key, WindowEvent},
    },
    types::{Vec2, Vec3},
    utils::load_text,
};

use crate::{
    camera_controller::FreeCameraController,
    color_obj::{load_obj_color, ColorObject, ColorVertex},
};

lazy_static::lazy_static! {
    #[derive(Debug)]
    pub static ref SHADER_CODE_MUTEX: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
}

pub struct Application {
    last_time: web_time::Instant,
    time: f32,
    cube: Object<Instance>,
    road: ColorObject,
    ground: Object<Instance>,
    shader: ShaderHandle,
    camera: BindGroupHandle<MatrixCameraBindGroup>,
    depth_texture: TextureHandle,
}

const REFERENCE_DIAGONAL: f32 = 2202.0;
const ORTHO_WIDTH: f32 = 20.0;
const ORTHO_HEIGHT: f32 = 20.0;
const ORTHO_NEAR: f32 = 0.003;
const ORTHO_FAR: f32 = 1000.0;

impl Application {
    pub async fn new(engine: &mut Engine) -> Self {
        let (aspect, diagonal) = {
            let size = engine.renderer.size();
            let size = Vec2::new(size.x as f32, size.y as f32);
            (size.x / size.y, (size.x * size.x + size.y * size.y).sqrt())
        };
        let controller = FreeCameraController {
            pan_speed: 0.02 * (diagonal / REFERENCE_DIAGONAL),
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
        *camera.position() = Vec3::new(0.0, 10.0, 0.0);
        *camera.direction() = Vec3::new(1.0, -1.0, 1.0).normalize();
        let camera = create_typed_bind_group(engine.renderer.as_mut(), camera);

        let shader: ShaderHandle = engine.renderer.create_shader(
            ShaderDescriptor::default()
                .with_source(jandering_engine::core::shader::ShaderSource::Code(
                    include_str!("../res/shaders/shader.wgsl").to_string(),
                ))
                .with_descriptors(vec![ColorVertex::desc(), Instance::desc()])
                .with_bind_group_layouts(vec![MatrixCameraBindGroup::get_layout()])
                .with_depth(true)
                .with_backface_culling(false),
        );

        let instances = (-10..10)
            .flat_map(|x| {
                (-10..10)
                    .map(|y| {
                        Instance::default().translate(Vec3::new(x as f32, 0.0, y as f32) * 10.0)
                    })
                    .collect::<Vec<Instance>>()
            })
            .collect();

        let cube = Object::from_obj(
            include_str!("../res/cube.obj"),
            engine.renderer.as_mut(),
            instances,
        );

        let road_instances = (-10..10)
            .flat_map(|x| {
                (-10..10)
                    .map(|y| {
                        Instance::default().translate(Vec3::new(x as f32, 0.0, y as f32) * 10.0)
                    })
                    .collect::<Vec<Instance>>()
            })
            .collect();

        let road = ColorObject::from_obj(
            include_str!("../res/road.obj"),
            engine.renderer.as_mut(),
            road_instances,
        );

        let ground = Object::from_obj(
            include_str!("../res/ground.obj"),
            engine.renderer.as_mut(),
            vec![Instance::default()
                .translate(Vec3::new(0.0, -0.5, 0.0))
                .scale(1.0)],
        );

        let depth_texture = engine.renderer.create_texture(TextureDescriptor {
            size: engine.renderer.size(),
            format: TextureFormat::Depth32F,
            ..Default::default()
        });

        Self {
            last_time: web_time::Instant::now(),
            time: 0.0,
            cube,
            road,
            ground,
            shader,
            camera,
            depth_texture,
        }
    }
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
                    .with_backface_culling(false),
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
    }

    fn on_render(&mut self, renderer: &mut Box<dyn Renderer>) {
        let camera = get_typed_bind_group(renderer.as_ref(), self.camera).unwrap();
        renderer.write_bind_group(self.camera.into(), &camera.get_data());

        renderer
            .new_pass()
            .with_depth(self.depth_texture, Some(1.0))
            .with_clear_color(0.2, 0.5, 1.0)
            .set_shader(self.shader)
            .bind(0, self.camera.into())
            .render(&[&self.ground, &self.road, &self.cube])
            .submit();
    }
}
