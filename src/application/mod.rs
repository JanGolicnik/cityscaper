use jandering_engine::{
    bind_group::{camera::free::MatrixCameraBindGroup, texture::TextureBindGroup, BindGroup},
    engine::{Engine, EngineContext},
    event_handler::EventHandler,
    object::{Instance, Object},
    render_pass::RenderPassTrait,
    renderer::{BindGroupHandle, Janderer, Renderer, ShaderHandle, TextureHandle},
    texture::{TextureDescriptor, TextureFormat},
    types::{Vec2, Vec3},
    utils::load_text,
    window::{
        FpsPreference, Window, WindowConfig, WindowEvent, WindowHandle, WindowManager,
        WindowManagerTrait, WindowTrait,
    },
};
use logic::{create_plant, create_plant_data};
use rand::rngs::ThreadRng;
use setup::{create_camera, create_objects, create_shaders, create_textures};

use crate::{
    color_obj::AgeObject, cylinder, l_system::config::LConfig, render_data::RenderDataBindGroup,
};

pub mod logic;
pub mod setup;
pub struct Application {
    last_time: std::time::Instant,

    main_window_handle: WindowHandle,
    input_window_handle: WindowHandle,
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
}

const N_DUST: u32 = 60;
const N_GRASS: u32 = 5000;

const REFERENCE_DIAGONAL: f32 = 2202.0;
const ORTHO_WIDTH: f32 = 2.0;
const ORTHO_HEIGHT: f32 = ORTHO_WIDTH;
const ORTHO_NEAR: f32 = 0.003;
const ORTHO_FAR: f32 = 1000.0;

impl Application {
    pub async fn new(engine: &mut Engine<Application>) -> Self {
        let (main_window_handle, input_window_handle) = {
            let window_manager = engine.window_manager();

            let main_window_handle = window_manager.create_window(
                WindowConfig::default()
                    .with_cursor(true)
                    .with_auto_resolution()
                    .with_title("real window")
                    .with_cursor(true)
                    .with_fps_preference(FpsPreference::Exact(30)),
            );

            let input_window_handle = window_manager.create_window(
                WindowConfig::default()
                    .with_resolution(200, 300)
                    .with_title("temp")
                    .with_cursor(true)
                    .with_decorations(false)
                    .with_transparency(true),
            );

            (main_window_handle, input_window_handle)
        };
        let renderer = &mut engine.renderer;
        let mut rng = rand::thread_rng();
        let (shader, floor_shader, grass_shader, dust_shader) = create_shaders(renderer).await;

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

        Self {
            main_window_handle,
            input_window_handle,
            last_time: std::time::Instant::now(),
            time: 0.0,
            shader,
            camera: BindGroupHandle(0, std::marker::PhantomData::<MatrixCameraBindGroup>),
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
        }
    }

    fn update_main_window(&mut self, context: &mut EngineContext) {
        let window = context
            .window_manager
            .get_window(self.main_window_handle)
            .unwrap();

        if window
            .events()
            .matches(|e| matches!(e, WindowEvent::Resized(_)))
        {
            let size = window.size();
            let aspect = { size.0 as f32 / size.1 as f32 };
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
                    size: size.into(),
                    format: TextureFormat::Depth32F,
                    ..Default::default()
                },
                self.depth_texture,
            );
        }

        let render_data = context
            .renderer
            .get_typed_bind_group_mut(self.render_data)
            .unwrap();
        render_data.data.time = self.time;
        render_data.data.wind_strength = 0.002 + (self.time * 0.2).sin().powf(4.0).max(0.0) * 0.01;
    }

    fn update_input_window(&mut self, context: &mut EngineContext) {
        let camera = context
            .renderer
            .get_typed_bind_group_mut(self.camera)
            .unwrap();

        let input_window_position = {
            let (width, height) = context
                .window_manager
                .get_window(self.main_window_handle)
                .unwrap()
                .size();

            let bottom_vertex = self.plant.vertices.first().unwrap();
            let camera_matrix = camera.matrix();

            let clip_pos = camera_matrix * Vec3::from(bottom_vertex.position).extend(1.0);
            let mut normalized_pos = Vec2::new(clip_pos.x, clip_pos.y) * 0.5 + 0.5;
            normalized_pos.y = 1.0 - normalized_pos.y;
            let pixel_pos = normalized_pos * Vec2::new(width as f32, height as f32);
            pixel_pos.round()
        };

        let window = context
            .window_manager
            .get_window(self.input_window_handle)
            .unwrap();

        window.set_absolute_position(
            input_window_position.x as i32 - window.width() as i32 / 2,
            input_window_position.y as i32 - window.height() as i32,
        );

        let current_time = std::time::Instant::now();
        let dt = (current_time - self.last_time).as_secs_f32();
        self.last_time = current_time;
        self.time += dt;

        camera.update(window.events(), dt);

        self.update_dust(dt, context.renderer);

        let instance = self.plant.instances.first_mut().unwrap();

        if window
            .events()
            .is_mouse_pressed(jandering_engine::window::MouseButton::Left)
        {
            *instance = instance.resize(-0.1);
        }

        let mut size = instance.size();

        let old_size = size;
        size += (Vec3::ONE - size) * (1.0 - (-15.0 * dt).exp());
        *instance = instance.set_size(size);

        if old_size.max_element() < 0.865 {
            let instance = *instance;
            self.plant = create_plant(context.renderer, &self.l_config, &mut rand::thread_rng());
            *self.plant.instances.first_mut().unwrap() = instance;
        } else {
            self.plant.update(context.renderer);
        }
    }

    fn put_input_window_above_icons(&mut self, input_window: &mut Window) {
        unsafe extern "system" fn enum_windows_proc(
            hwnd: windows::Win32::Foundation::HWND,
            l_param: windows::Win32::Foundation::LPARAM,
        ) -> windows::Win32::Foundation::BOOL {
            let out_hwnd = l_param.0 as *mut windows::Win32::Foundation::HWND;

            let p: windows::Win32::Foundation::HWND =
                windows::Win32::UI::WindowsAndMessaging::FindWindowExA(
                    hwnd,
                    None,
                    windows::core::s!("SHELLDLL_DefView"),
                    None,
                );

            if p.0 != 0 {
                *out_hwnd = p;
            }
            windows::Win32::Foundation::TRUE
        }

        match input_window.get_raw_window_handle() {
            raw_window_handle::RawWindowHandle::Win32(handle) => unsafe {
                let mut hwnd = windows::Win32::Foundation::HWND(0);

                windows::Win32::UI::WindowsAndMessaging::EnumWindows(
                    Some(enum_windows_proc),
                    windows::Win32::Foundation::LPARAM(
                        &mut hwnd as *mut windows::Win32::Foundation::HWND as isize,
                    ),
                )
                .unwrap();

                windows::Win32::UI::WindowsAndMessaging::SetParent(
                    windows::Win32::Foundation::HWND(handle.hwnd as isize),
                    hwnd,
                );
            },
            _ => todo!(),
        }
    }
}

impl EventHandler for Application {
    fn init(&mut self, renderer: &mut Renderer, window_manager: &mut WindowManager) {
        renderer.register_window(self.main_window_handle, window_manager);

        renderer.register_window(self.input_window_handle, window_manager);

        let window = window_manager.get_window(self.main_window_handle).unwrap();

        window.set_as_desktop();
        let resolution = window.size();

        self.camera = create_camera(
            renderer,
            Vec2::new(resolution.0 as f32, resolution.1 as f32),
        );

        renderer.re_create_texture(
            TextureDescriptor {
                size: resolution.into(),
                format: TextureFormat::Depth32F,
                ..Default::default()
            },
            self.depth_texture,
        );

        self.put_input_window_above_icons(
            window_manager.get_window(self.input_window_handle).unwrap(),
        );
    }

    fn on_update(&mut self, context: &mut EngineContext) {
        self.update_main_window(context);
        self.update_input_window(context);
    }

    fn on_render(&mut self, renderer: &mut Renderer, _: WindowHandle, _: &mut WindowManager) {
        let camera = renderer.get_typed_bind_group(self.camera).unwrap();
        renderer.write_bind_group(self.camera.into(), &camera.get_data());

        let render_data = renderer.get_typed_bind_group(self.render_data).unwrap();
        renderer.write_bind_group(self.render_data.into(), &render_data.get_data());
        renderer
            .new_pass(self.main_window_handle)
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

        renderer
            .new_pass(self.input_window_handle)
            .with_clear_color(0.2, 0.5, 1.0)
            .with_alpha(0.0)
            .render_empty()
            .submit();
    }
}
