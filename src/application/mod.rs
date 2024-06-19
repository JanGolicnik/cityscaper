use jandering_engine::{
    bind_group::{camera::free::MatrixCameraBindGroup, texture::TextureBindGroup, BindGroup},
    engine::{EngineContext, EventHandlerBuilder},
    event_handler::EventHandler,
    object::{Instance, Object, Vertex},
    renderer::{BindGroupHandle, Janderer, Renderer, ShaderHandle, TextureHandle},
    shader::ShaderDescriptor,
    texture::{TextureDescriptor, TextureFormat},
    utils::load_text,
    window::{Key, WindowEvent, WindowTrait},
};
use logic::create_plant;
use rand::{rngs::ThreadRng, thread_rng};

use self::setup::{create_camera, create_objects, create_shaders, create_textures};
use crate::{
    color_obj::AgeObject, cylinder, l_system::config::LConfig, render_data::RenderDataBindGroup,
};

pub mod logic;
pub mod setup;

pub struct ApplicationBuilder {
    shader_source: String,
}

impl ApplicationBuilder {
    pub async fn new() -> Self {
        let shader_source = load_text(jandering_engine::utils::FilePath::FileName(
            "shaders/shader.wgsl",
        ))
        .await
        .unwrap();
        Self { shader_source }
    }
}

impl EventHandlerBuilder<Application> for ApplicationBuilder {
    fn build(self, renderer: &mut Renderer) -> Application {
        Application::new(renderer, self)
    }
}

pub struct Application {
    last_time: std::time::Instant,
    time: f32,
    shader: ShaderHandle,
    floor_shader: ShaderHandle,
    grass_shader: ShaderHandle,
    camera: BindGroupHandle<MatrixCameraBindGroup>,
    depth_texture: TextureHandle,

    plant: AgeObject,
    l_config: LConfig,
    floor: Object<Instance>,

    dust: AgeObject,
    dust_shader: ShaderHandle,
    grass: AgeObject,
    noise_texture: BindGroupHandle<TextureBindGroup>,

    lut_texture: BindGroupHandle<TextureBindGroup>,
    lut_texture_linear: BindGroupHandle<TextureBindGroup>,

    render_data: BindGroupHandle<RenderDataBindGroup>,

    rng: ThreadRng,

    first_frame: bool,
}

const N_DUST: u32 = 60;
const N_GRASS: u32 = 5000;

const REFERENCE_DIAGONAL: f32 = 2202.0;
const ORTHO_WIDTH: f32 = 2.0;
const ORTHO_HEIGHT: f32 = ORTHO_WIDTH;
const ORTHO_NEAR: f32 = 0.003;
const ORTHO_FAR: f32 = 1000.0;

impl Application {
    pub fn new(renderer: &mut Renderer, builder: ApplicationBuilder) -> Self {
        let mut rng = thread_rng();
        let (shader, floor_shader, grass_shader, dust_shader) = create_shaders(renderer, &builder);

        let (depth_texture, noise_image, noise_texture, lut_texture, lut_texture_linear) =
            create_textures(renderer);

        let (floor, dust, grass) = create_objects(renderer, &mut rng, &noise_image);

        let l_config_json = pollster::block_on(load_text(
            jandering_engine::utils::FilePath::FileName("systems/initial.json"),
        ))
        .unwrap();
        let mut l_config = LConfig::from_json(l_config_json).unwrap();
        l_config.rules.iterations = 10;
        let plant = create_plant(renderer, &l_config, &mut rng);

        let render_data = RenderDataBindGroup::new(renderer);
        let render_data = renderer.create_typed_bind_group(render_data);

        let camera = create_camera(renderer, &mut rng);

        Self {
            last_time: std::time::Instant::now(),
            time: 0.0,
            shader,
            camera,
            depth_texture,

            grass_shader,
            floor_shader,

            plant,
            l_config,
            floor,

            dust,
            dust_shader,
            grass,
            noise_texture,

            lut_texture,
            lut_texture_linear,

            render_data,

            rng,

            first_frame: true,
        }
    }
}

impl EventHandler for Application {
    fn on_update(&mut self, context: &mut EngineContext) {
        if self.first_frame {
            context.window.set_as_desktop();
            self.first_frame = false;
        }

        let current_time = std::time::Instant::now();
        let dt = (current_time - self.last_time).as_secs_f32();
        self.last_time = current_time;
        self.time += dt;

        if context.events.is_pressed(Key::V) {
            let code = pollster::block_on(load_text(jandering_engine::utils::FilePath::FileName(
                "shaders/shader.wgsl",
            )))
            .unwrap();
            context.renderer.create_shader_at(
                ShaderDescriptor::default()
                    .with_source(jandering_engine::shader::ShaderSource::Code(code))
                    .with_descriptors(vec![Vertex::desc(), Instance::desc()])
                    .with_bind_group_layouts(vec![MatrixCameraBindGroup::get_layout()])
                    .with_depth(true)
                    .with_backface_culling(true),
                self.shader,
            );

            let l_config_json = pollster::block_on(load_text(
                jandering_engine::utils::FilePath::FileName("systems/initial.rs"),
            ))
            .unwrap();
            self.l_config = LConfig::from_json(l_config_json).unwrap();
        }

        if context
            .events
            .matches(|e| matches!(e, WindowEvent::Resized(_)))
        {
            let aspect = {
                let size = context.renderer.size();
                size.x as f32 / size.y as f32
            };
            let camera = context
                .renderer
                .get_typed_bind_group_mut(self.camera)
                .unwrap();
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

        let camera = context
            .renderer
            .get_typed_bind_group_mut(self.camera)
            .unwrap();
        camera.update(context.events, dt);

        self.update_dust(dt, context.renderer);

        let render_data = context
            .renderer
            .get_typed_bind_group_mut(self.render_data)
            .unwrap();
        render_data.data.time = self.time;
        render_data.data.wind_strength = 0.002 + (self.time * 0.2).sin().powf(4.0).max(0.0) * 0.01;
    }

    fn on_render(&mut self, renderer: &mut Renderer) {
        let camera = renderer.get_typed_bind_group(self.camera).unwrap();
        renderer.write_bind_group(self.camera.into(), &camera.get_data());

        let render_data = renderer.get_typed_bind_group(self.render_data).unwrap();
        renderer.write_bind_group(self.render_data.into(), &render_data.get_data());

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
            .render(&[&self.plant])
            .set_shader(self.dust_shader)
            .render(&[&self.dust])
            .bind(3, self.lut_texture_linear.into())
            .set_shader(self.grass_shader)
            .render(&[&self.grass])
            .submit();
    }
}
