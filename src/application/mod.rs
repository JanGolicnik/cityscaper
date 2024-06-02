use jandering_engine::{
    core::{
        bind_group::{
            camera::free::{CameraController, FreeCameraController, MatrixCameraBindGroup},
            texture::TextureBindGroup,
            BindGroup,
        },
        engine::{Engine, EngineContext},
        event_handler::EventHandler,
        object::{Instance, Object, Renderable, Vertex},
        renderer::{
            create_typed_bind_group, get_typed_bind_group, get_typed_bind_group_mut,
            BindGroupHandle, Renderer, SamplerHandle, ShaderHandle, TextureHandle,
        },
        shader::ShaderDescriptor,
        texture::{TextureDescriptor, TextureFormat},
        window::{Key, WindowEvent},
    },
    types::Vec2,
    utils::load_text,
};
use rand::{rngs::ThreadRng, thread_rng};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    color_obj::AgeObject, cylinder, image::Image, l_system::config::LConfig,
    render_data::RenderDataBindGroup,
};

use self::setup::{
    create_camera, create_lut_textures, create_objects, create_shaders, create_textures,
};

pub mod logic;
pub mod setup;

lazy_static::lazy_static! {
    #[derive(Debug)]
    pub static ref SHADER_CODE_MUTEX: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
}

type Plants = HashMap<(i32, i32), AgeObject>;

pub struct Application {
    last_time: web_time::Instant,
    time: f32,
    shader: ShaderHandle,
    floor_shader: ShaderHandle,
    grass_shader: ShaderHandle,
    camera: BindGroupHandle<MatrixCameraBindGroup>,
    camera_controller: Box<dyn CameraController>,
    depth_texture: TextureHandle,

    plants: Plants,
    l_config: LConfig,
    floor: Object<Instance>,

    dust: AgeObject,
    dust_shader: ShaderHandle,
    grass: AgeObject,
    noise_image: Image,
    noise_texture: BindGroupHandle<TextureBindGroup>,

    lut_texture: BindGroupHandle<TextureBindGroup>,
    lut_texture_linear: BindGroupHandle<TextureBindGroup>,
    lut_sampler: SamplerHandle,

    render_data: BindGroupHandle<RenderDataBindGroup>,

    rng: ThreadRng,
}

const N_DUST: u32 = 60;
const N_GRASS: u32 = 5000;

const REFERENCE_DIAGONAL: f32 = 2202.0;
const ORTHO_WIDTH: f32 = 2.0;
const ORTHO_HEIGHT: f32 = ORTHO_WIDTH;
const ORTHO_NEAR: f32 = 0.003;
const ORTHO_FAR: f32 = 1000.0;

impl Application {
    pub async fn new(engine: &mut Engine) -> Self {
        let (shader, floor_shader, grass_shader, dust_shader) =
            create_shaders(engine.renderer.as_mut()).await;

        let (
            depth_texture,
            noise_image,
            noise_texture,
            lut_sampler,
            lut_texture,
            lut_texture_linear,
        ) = create_textures(engine.renderer.as_mut()).await;

        let (plants, floor, dust, grass) = create_objects(engine.renderer.as_mut());

        let json = load_text(jandering_engine::utils::FilePath::FileName("lsystem.json"))
            .await
            .unwrap();
        let l_config = LConfig::from_json(json);

        let render_data = RenderDataBindGroup::new(engine.renderer.as_mut());
        let render_data = create_typed_bind_group(engine.renderer.as_mut(), render_data);

        let camera = create_camera(engine.renderer.as_mut());

        let rng = thread_rng();

        Self {
            last_time: web_time::Instant::now(),
            time: 0.0,
            shader,
            camera,
            camera_controller: Box::<FreeCameraController>::default(),
            depth_texture,

            grass_shader,
            floor_shader,

            plants,
            l_config,
            floor,

            dust,
            dust_shader,
            grass,
            noise_image,
            noise_texture,

            lut_texture,
            lut_texture_linear,
            lut_sampler,

            render_data,

            rng,
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
                    .with_backface_culling(true),
                self.shader,
            );
            *guard = None;
        }

        if context.events.is_pressed(Key::V) {
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

        if context.events.is_pressed(Key::F) {
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

        if context.events.is_pressed(Key::G) {
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
        self.update_grass(context.renderer.as_mut());

        create_lut_textures(
            context.renderer.as_mut(),
            Some(self.lut_texture),
            Some(self.lut_texture_linear),
            Some(self.lut_sampler),
        );

        self.update_iteration_count();

        let render_data =
            get_typed_bind_group_mut(context.renderer.as_mut(), self.render_data).unwrap();
        render_data.data.time = self.time;
        render_data.data.wind_strength = 0.002 + (self.time * 0.2).sin().powf(4.0).max(0.0) * 0.01;
    }

    fn on_render(&mut self, renderer: &mut Box<dyn Renderer>) {
        let camera = get_typed_bind_group(renderer.as_ref(), self.camera).unwrap();
        renderer.write_bind_group(self.camera.into(), &camera.get_data());

        let render_data = get_typed_bind_group(renderer.as_ref(), self.render_data).unwrap();
        renderer.write_bind_group(self.render_data.into(), &render_data.get_data());

        let plants = self
            .plants
            .values()
            .map(|e| e as &dyn Renderable)
            .collect::<Vec<_>>();

        renderer
            .new_pass()
            .with_depth(self.depth_texture, Some(1.0))
            .with_clear_color(0.2, 0.5, 1.0)
            .set_shader(self.floor_shader)
            .bind(0, self.camera.into())
            .bind(1, self.render_data.into())
            .bind(2, self.noise_texture.into())
            .bind(3, self.lut_texture.into())
            .render(&[&self.floor])
            .set_shader(self.shader)
            .render(&plants)
            .set_shader(self.dust_shader)
            .render(&[&self.dust])
            .bind(3, self.lut_texture_linear.into())
            .set_shader(self.grass_shader)
            .render(&[&self.grass])
            .submit();
    }
}
