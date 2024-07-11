use color::{ColorLut, ColorValue};
use is_none_or::IsNoneOr;
use jandering_engine::{
    bind_group::{camera::free::MatrixCameraBindGroup, texture::TextureBindGroup, BindGroup},
    engine::{Engine, EngineContext},
    event_handler::EventHandler,
    object::{Instance, Object},
    render_pass::RenderPassTrait,
    renderer::{BindGroupHandle, Janderer, Renderer, SamplerHandle, ShaderHandle, TextureHandle},
    texture::{TextureDescriptor, TextureFormat},
    types::{Vec2, Vec3},
    utils::load_text,
    window::{
        FpsPreference, Window, WindowConfig, WindowEvent, WindowHandle, WindowManager,
        WindowManagerTrait, WindowTrait,
    },
};
use logic::create_plant;
use rand::{rngs::ThreadRng, Rng};
use setup::{
    create_camera, create_objects, create_shaders, create_textures, re_create_lut_textures,
};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use crate::{
    color_obj::AgeObject, cylinder, l_system::config::LConfig, main,
    render_data::RenderDataBindGroup,
};

pub mod color;
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
    multisample_texture: TextureHandle,

    plant: AgeObject,
    l_config: LConfig,
    floor: Object<Instance>,

    dust: AgeObject,
    dust_shader: ShaderHandle,
    grass: AgeObject,
    noise_texture: BindGroupHandle<TextureBindGroup>,

    lut_texture: BindGroupHandle<TextureBindGroup>,
    lut_texture_linear: BindGroupHandle<TextureBindGroup>,
    lut_sampler: SamplerHandle,

    render_data: BindGroupHandle<RenderDataBindGroup>,

    rng: ThreadRng,

    system: sysinfo::System,
    machine: machine_info::Machine,

    plant_size: f32,

    cpu_samples: Vec<(std::time::Instant, f32)>,
    cpu_sample_timer: f32,

    ram_samples: Vec<(std::time::Instant, f32)>,
    ram_sample_timer: f32,

    gpu_samples: Vec<(std::time::Instant, f32)>,
    gpu_sample_timer: f32,

    plant_interpolation: f32,

    color_lut: ColorLut,
    original_color_lut: ColorLut,
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
                    .with_transparency(true)
                    .with_fps_preference(FpsPreference::Exact(30)),
            );

            (main_window_handle, input_window_handle)
        };
        let renderer = &mut engine.renderer;

        let (shader, floor_shader, grass_shader, dust_shader) = create_shaders(renderer).await;

        let lut_json = pollster::block_on(load_text(jandering_engine::utils::FilePath::FileName(
            "lut.json",
        )))
        .unwrap();
        let color_lut = ColorLut::new(&lut_json);

        let (
            depth_texture,
            multisample_texture,
            noise_image,
            noise_texture,
            lut_sampler,
            lut_texture,
            lut_texture_linear,
        ) = create_textures(renderer, &color_lut);

        let mut rng = rand::thread_rng();

        let (floor, dust, grass) = create_objects(renderer, &mut rng, &noise_image);

        let l_config_json = pollster::block_on(load_text(
            jandering_engine::utils::FilePath::FileName("systems/initial.json"),
        ))
        .unwrap();
        let mut l_config = LConfig::from_json(l_config_json).unwrap();
        l_config.rules.iterations = 0;
        let plant = create_plant(renderer, &l_config);
        l_config.rules.iterations = 10;

        let render_data = RenderDataBindGroup::new(renderer);
        let render_data = renderer.create_typed_bind_group(render_data);

        let system = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::new().with_ram()),
        );

        let machine = machine_info::Machine::new();

        let plant_interpolation = l_config.interpolation;

        Self {
            main_window_handle,
            input_window_handle,
            last_time: std::time::Instant::now(),
            time: 0.0,
            shader,
            camera: BindGroupHandle(0, std::marker::PhantomData::<MatrixCameraBindGroup>),
            depth_texture,
            multisample_texture,

            grass_shader,
            floor_shader,

            plant,
            l_config,
            floor,

            dust,
            dust_shader,
            grass,
            noise_texture,

            lut_sampler,
            lut_texture,
            lut_texture_linear,

            render_data,

            rng,

            system,
            machine,

            plant_size: 1.0,

            cpu_samples: Vec::new(),
            cpu_sample_timer: 0.0,

            ram_samples: Vec::new(),
            ram_sample_timer: 0.0,

            gpu_samples: Vec::new(),
            gpu_sample_timer: 0.0,

            plant_interpolation,

            original_color_lut: color_lut.clone(),
            color_lut
        }
    }

    fn get_average_gpu(&mut self, dt: f32) -> f32 {
        self.gpu_sample_timer -= dt;
        if self.gpu_sample_timer < 0.0 {
            for gpu in self.machine.graphics_status() {
                self.gpu_samples
                    .push((std::time::Instant::now(), gpu.gpu as f32 / 100.0));
            }
            self.gpu_sample_timer = 1.5;
        }

        let current_time = std::time::Instant::now();
        self.gpu_samples.retain(|(start_time, _)| {
            current_time.duration_since(*start_time).as_secs_f32() < 15.0
        });

        if self.gpu_samples.is_empty() {
            return 1.0;
        }

        self.gpu_samples
            .iter()
            .fold(0.0, |acc, (_, value)| acc + *value)
            / self.gpu_samples.len() as f32
    }

    fn get_average_ram(&mut self, dt: f32) -> f32 {
        self.ram_sample_timer -= dt;
        if self.ram_sample_timer < 0.0 {
            self.system.refresh_memory();

            let current_memory = self.system.used_memory();
            let max_memory = self.system.total_memory();

            self.ram_samples.push((
                std::time::Instant::now(),
                current_memory as f32 / max_memory as f32,
            ));

            self.ram_sample_timer = 0.1;
        }

        let current_time = std::time::Instant::now();
        self.ram_samples
            .retain(|(start_time, _)| current_time.duration_since(*start_time).as_secs_f32() < 5.0);

        if self.ram_samples.is_empty() {
            return 1.0;
        }

        self.ram_samples
            .iter()
            .fold(0.0, |acc, (_, value)| acc + *value)
            / self.ram_samples.len() as f32
    }

    fn get_average_cpu(&mut self, dt: f32) -> f32 {
        self.cpu_sample_timer -= dt;

        if self.cpu_sample_timer < 0.0 {
            self.system.refresh_cpu();
            for cpu in self.system.cpus() {
                self.cpu_samples
                    .push((std::time::Instant::now(), cpu.cpu_usage()));
            }

            self.cpu_sample_timer = 0.1;
        }

        let current_time = std::time::Instant::now();
        self.cpu_samples.retain(|(start_time, _)| {
            current_time
                .checked_duration_since(*start_time)
                .is_none_or(|time| time.as_secs_f32() < 5.0)
        });

        let average = self
            .cpu_samples
            .iter()
            .fold(0.0, |acc, (_, value)| acc + *value)
            / self.cpu_samples.len() as f32;
        average / 100.0
    }

    fn update_main_window(&mut self, context: &mut EngineContext, dt: f32) {
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
                    sample_count: 4,
                    ..Default::default()
                },
                self.depth_texture,
            );

            context.renderer.re_create_texture(
                TextureDescriptor {
                    size: size.into(),
                    sample_count: 4,
                    ..Default::default()
                },
                self.multisample_texture,
            );
        }

        let render_data = context
            .renderer
            .get_typed_bind_group_mut(self.render_data)
            .unwrap();

        render_data.data.time = self.time;
        render_data.data.wind_strength = 0.002 + (self.time * 0.2).sin().powf(4.0).max(0.0) * 0.01;

        let average_cpu = self.get_average_cpu(dt);
        let target_interpolation = average_cpu * 0.5 + 0.5;
        self.plant_interpolation +=
            (target_interpolation - self.plant_interpolation) * (1.0 - (-0.3 * dt).exp());

        let average_ram = self.get_average_ram(dt);
        let width_mod = average_ram + 0.5;

        let average_gpu = self.get_average_gpu(dt);
        for (i, color )in self.color_lut.colors.iter_mut().enumerate() {
            if let ColorValue::HSL { value } = &mut color.color {
                value[0] = (value[0] + average_gpu * average_gpu * dt * 30.0) % 360.0;
                if let ColorValue::HSL { value: original_value } = self.original_color_lut.colors[i].color {
                    value[1] = original_value[1] * (average_gpu * 0.5 + 0.5);
                }
            }
        }

        re_create_lut_textures(
            context.renderer,
            &self.color_lut,
            self.lut_texture,
            self.lut_texture_linear,
            self.lut_sampler,
        );

        if (self.plant_interpolation - self.l_config.interpolation).abs() > 0.001
            || (width_mod - self.l_config.rendering.width_mod.unwrap_or(0.0)).abs() > 0.001
        {
            self.l_config.interpolation = self.plant_interpolation;
            self.l_config.rendering.width_mod = Some(width_mod);
            self.l_config.reseed(self.l_config.seed);
            self.plant = create_plant(context.renderer, &self.l_config);
        }
    }

    fn update_input_window(&mut self, context: &mut EngineContext, dt: f32) {
        let camera = context
            .renderer
            .get_typed_bind_group_mut(self.camera)
            .unwrap();

        let main_window_size = context
            .window_manager
            .get_window(self.main_window_handle)
            .unwrap()
            .size();

        let main_window_position = context
            .window_manager
            .get_window(self.main_window_handle)
            .unwrap()
            .position();

        let window = context
            .window_manager
            .get_window(self.input_window_handle)
            .unwrap();

        if let Some(bottom_vertex) = self.plant.vertices.first() {
            let input_window_position = {
                let camera_matrix = camera.matrix();

                let clip_pos = camera_matrix * Vec3::from(bottom_vertex.position).extend(1.0);
                let mut normalized_pos = Vec2::new(clip_pos.x, clip_pos.y) * 0.5 + 0.5;
                normalized_pos.y = 1.0 - normalized_pos.y;
                let pixel_pos = normalized_pos
                    * Vec2::new(main_window_size.0 as f32, main_window_size.1 as f32);
                pixel_pos.round()
            };

            window.set_absolute_position(
                input_window_position.x as i32 - window.width() as i32 / 2 - main_window_position.0,
                input_window_position.y as i32 - window.height() as i32 - main_window_position.1,
            );
        }

        camera.update(window.events(), dt);

        self.update_dust(dt, context.renderer);

        if window
            .events()
            .is_mouse_pressed(jandering_engine::window::MouseButton::Left)
        {
            self.plant_size -= 0.1;
        }

        let old_size = self.plant_size;
        self.plant_size += (1.0 - self.plant_size) * (1.0 - (-15.0 * dt).exp());

        let instance = self.plant.instances.first_mut().unwrap();
        *instance = instance.set_size(Vec3::splat(self.plant_size));

        if old_size < 0.865 {
            let instance = *instance;
            self.l_config.randomize_rule_sets(Some(2));
            self.l_config.reseed(rand::thread_rng().gen::<u64>());
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
            let p: windows::Win32::Foundation::HWND =
                windows::Win32::UI::WindowsAndMessaging::FindWindowExA(
                    hwnd,
                    None,
                    windows::core::s!("SHELLDLL_DefView"),
                    None,
                );

            if p.0 != 0 {
                let out_hwnd = l_param.0 as *mut windows::Win32::Foundation::HWND;
                *out_hwnd = hwnd;
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
                sample_count: 4,
                ..Default::default()
            },
            self.depth_texture,
        );

        renderer.re_create_texture(
            TextureDescriptor {
                size: resolution.into(),
                sample_count: 4,
                ..Default::default()
            },
            self.multisample_texture,
        );

        self.put_input_window_above_icons(
            window_manager.get_window(self.input_window_handle).unwrap(),
        );
    }

    fn on_update(&mut self, context: &mut EngineContext) {
        let current_time = std::time::Instant::now();
        let dt = (current_time - self.last_time).as_secs_f32();
        self.last_time = current_time;
        self.time += dt;

        self.update_main_window(context, dt);
        self.update_input_window(context, dt);
    }

    fn on_render(&mut self, renderer: &mut Renderer, _: WindowHandle, _: &mut WindowManager) {
        let camera = renderer.get_typed_bind_group(self.camera).unwrap();
        renderer.write_bind_group(self.camera.into(), &camera.get_data());

        let render_data = renderer.get_typed_bind_group(self.render_data).unwrap();
        renderer.write_bind_group(self.render_data.into(), &render_data.get_data());
        renderer
            .new_pass(self.main_window_handle)
            .with_depth(self.depth_texture, Some(1.0))
            .with_target_texture_resolve(self.multisample_texture, None)
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
