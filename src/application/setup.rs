use image::GenericImageView;
use jandering_engine::{
    bind_group::{
        camera::free::{CameraController, MatrixCameraBindGroup},
        texture::TextureBindGroup,
    },
    object::{Instance, Object, Vertex},
    renderer::{BindGroupHandle, Janderer, Renderer, SamplerHandle, ShaderHandle, TextureHandle},
    shader::ShaderDescriptor,
    texture::{sampler::SamplerDescriptor, TextureDescriptor, TextureFormat},
    types::{Mat4, Qua, UVec2, Vec2, Vec3},
    utils::load_text,
};
use rand::{rngs::ThreadRng, Rng};

use crate::{
    camera_controller::IsometricCameraController,
    color_obj::{AgeObject, AgeVertex},
    image::Image,
};

use super::{
    logic::{parse_lut, place_pos_on_heightmap},
    RenderDataBindGroup, N_DUST, N_GRASS, ORTHO_FAR, ORTHO_HEIGHT, ORTHO_NEAR, ORTHO_WIDTH,
    REFERENCE_DIAGONAL,
};
const GRASS_RANGE: f32 = 2.75;
const GRASS_ITERATIONS: u32 = 12;
const GRASS_HEIGHT: f32 = 0.1;
const GRASS_WIDTH: f32 = 0.0075;

pub fn create_camera(
    renderer: &mut Renderer,
    size: Vec2,
) -> BindGroupHandle<MatrixCameraBindGroup> {
    let aspect = size.x / size.y;
    let diagonal = (size.x * size.x + size.y * size.y).sqrt();
    let controller = IsometricCameraController {
        pan_speed: 0.002 * (diagonal / REFERENCE_DIAGONAL),
        ..Default::default()
    };
    let controller: Box<dyn CameraController> = Box::new(controller);
    let mut camera = MatrixCameraBindGroup::with_controller(controller);
    // let mut camera = MatrixCameraBindGroup::default();
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
    renderer.create_typed_bind_group(camera)
}
pub fn create_grass(rng: &mut ThreadRng, noise_image: &Image) -> Vec<Instance> {
    (0..N_GRASS)
        .map(|_| {
            let dist = rng.gen::<f32>();
            let angle = rng.gen_range(0.0f32..360.0f32);

            let pos_2d = Vec2::from_angle(angle.to_radians()) * dist * GRASS_RANGE;
            let mut pos = Vec3::new(pos_2d.x, 0.0, pos_2d.y);

            pos = place_pos_on_heightmap(pos, GRASS_ITERATIONS, noise_image, rng);

            let scale_mod = 0.7 + noise_image.sample(pos.x, pos.y) * 0.6;
            let mut scale = Vec3::new(GRASS_WIDTH, GRASS_HEIGHT, 1.0) * scale_mod;
            scale *= 1.0 - (pos.length() / GRASS_RANGE) * 0.5;

            let mat = Mat4::from_scale_rotation_translation(scale, Qua::default(), pos);
            Instance {
                model: mat,
                inv_model: mat.inverse(),
            }
        })
        .collect()
}

pub fn create_objects(
    renderer: &mut Renderer,
    rng: &mut ThreadRng,
    noise_image: &Image,
) -> (Object<Instance>, AgeObject, AgeObject) {
    let floor = Object::quad(
        renderer,
        vec![Instance::default()
            .rotate(90.0f32.to_radians(), Vec3::X)
            .set_size(Vec3::splat(100.0))],
    );

    let dust_instances = (0..N_DUST)
        .map(|_| Instance::default().translate(Vec3::splat(-1000.0)))
        .collect();
    let dust = AgeObject::quad(renderer, 0.3, dust_instances);

    let grass_instances = create_grass(rng, noise_image);
    let grass = AgeObject::quad(renderer, 1.0, grass_instances);

    (floor, dust, grass)
}

pub fn create_textures(
    renderer: &mut Renderer,
) -> (
    TextureHandle,
    TextureHandle,
    Image,
    BindGroupHandle<TextureBindGroup>,
    BindGroupHandle<TextureBindGroup>,
    BindGroupHandle<TextureBindGroup>,
) {
    let (lut_texture, lut_texture_linear) = create_lut_textures(renderer, None, None, None);
    let depth_texture = renderer.create_texture(TextureDescriptor {
        size: (100, 100).into(),
        format: TextureFormat::Depth32F,
        sample_count: 4,
        ..Default::default()
    });
    let multisample_texture = renderer.create_texture(TextureDescriptor {
        size: (100, 100).into(),
        sample_count: 4,
        ..Default::default()
    });
    let noise_image = image::load_from_memory(include_bytes!("../../res/noise.png")).unwrap();
    let noise_texture = {
        let tex_sampler = renderer.create_sampler(SamplerDescriptor {
            address_mode: jandering_engine::texture::sampler::SamplerAddressMode::Repeat,
            ..Default::default()
        });
        let noise_handle = renderer.create_texture(TextureDescriptor {
            data: Some(&noise_image.to_rgba8()),
            size: noise_image.dimensions().into(),
            format: TextureFormat::Rgba8U,
            ..Default::default()
        });
        let noise_texture = TextureBindGroup::new(renderer, noise_handle, tex_sampler);
        renderer.create_typed_bind_group(noise_texture)
    };
    let noise_image = Image::new(noise_image.to_rgb32f(), 0.1);

    (
        depth_texture,
        multisample_texture,
        noise_image,
        noise_texture,
        lut_texture,
        lut_texture_linear,
    )
}

pub async fn create_shaders(
    renderer: &mut Renderer,
) -> (ShaderHandle, ShaderHandle, ShaderHandle, ShaderHandle) {
    let shader_source = load_text(jandering_engine::utils::FilePath::FileName(
        "shaders/shader.wgsl",
    ))
    .await
    .unwrap();
    let descriptor = ShaderDescriptor::default()
        .with_source(jandering_engine::shader::ShaderSource::Code(
            shader_source.clone(),
        ))
        .with_descriptors(vec![AgeVertex::desc(), Instance::desc()])
        .with_depth(true)
        .with_backface_culling(false)
        .with_multisample(4)
        .with_bind_group_layouts(vec![
            MatrixCameraBindGroup::get_layout(),
            RenderDataBindGroup::get_layout(),
            TextureBindGroup::get_layout(),
            TextureBindGroup::get_layout(),
        ]);
    let shader: ShaderHandle =
        renderer.create_shader(descriptor.clone().with_fs_entry("fs_color_object"));
    let floor_shader: ShaderHandle = renderer.create_shader(
        descriptor
            .clone()
            .with_descriptors(vec![Vertex::desc(), Instance::desc()])
            .with_fs_entry("fs_floor"),
    );
    let grass_shader: ShaderHandle =
        renderer.create_shader(descriptor.clone().with_fs_entry("fs_grass"));
    let dust_shader: ShaderHandle =
        renderer.create_shader(descriptor.clone().with_fs_entry("fs_dust"));

    (shader, floor_shader, grass_shader, dust_shader)
}

pub fn create_lut_textures(
    renderer: &mut Renderer,
    lut_handle: Option<BindGroupHandle<TextureBindGroup>>,
    lut_handle_linear: Option<BindGroupHandle<TextureBindGroup>>,
    mut lut_sampler: Option<SamplerHandle>,
) -> (
    BindGroupHandle<TextureBindGroup>,
    BindGroupHandle<TextureBindGroup>,
) {
    if lut_sampler.is_none() {
        lut_sampler = Some(renderer.create_sampler(SamplerDescriptor {
            address_mode: jandering_engine::texture::sampler::SamplerAddressMode::Clamp,
            ..Default::default()
        }));
    }

    let lut_json = pollster::block_on(load_text(jandering_engine::utils::FilePath::FileName(
        "lut.json",
    )))
    .unwrap();

    let data = parse_lut(&lut_json, false)
        .unwrap_or_default()
        .iter()
        .flat_map(|e| {
            [
                (e.x * 255.0) as u8,
                (e.y * 255.0) as u8,
                (e.z * 255.0) as u8,
                255,
            ]
        })
        .collect::<Vec<_>>();
    let mut desc = TextureDescriptor {
        data: if data.is_empty() { None } else { Some(&data) },
        size: UVec2 {
            x: (data.len() as u32 / 4).max(1),
            y: 1,
        },
        format: TextureFormat::Rgba8U,
        ..Default::default()
    };

    let lut_texture = if let Some(handle) = lut_handle {
        let texture_handle = renderer
            .get_typed_bind_group(handle)
            .unwrap()
            .texture_handle;
        renderer.re_create_texture(desc.clone(), texture_handle);
        let texture = TextureBindGroup::new(renderer, texture_handle, lut_sampler.unwrap());

        renderer.create_typed_bind_group_at(texture, handle);
        handle
    } else {
        let handle = renderer.create_texture(desc.clone());
        let texture = TextureBindGroup::new(renderer, handle, lut_sampler.unwrap());
        renderer.create_typed_bind_group(texture)
    };

    let data = parse_lut(&lut_json, true)
        .unwrap_or_default()
        .iter()
        .flat_map(|e| {
            [
                (e.x * 255.0) as u8,
                (e.y * 255.0) as u8,
                (e.z * 255.0) as u8,
                255,
            ]
        })
        .collect::<Vec<_>>();

    desc.data = if data.is_empty() { None } else { Some(&data) };
    desc.size.x = (data.len() as u32 / 4).max(1);

    let lut_texture_linear = if let Some(handle) = lut_handle_linear {
        let texture_handle = renderer
            .get_typed_bind_group(handle)
            .unwrap()
            .texture_handle;
        renderer.re_create_texture(desc, texture_handle);
        let texture = TextureBindGroup::new(renderer, texture_handle, lut_sampler.unwrap());

        renderer.create_typed_bind_group_at(texture, handle);
        handle
    } else {
        let handle = renderer.create_texture(desc);
        let texture = TextureBindGroup::new(renderer, handle, lut_sampler.unwrap());
        renderer.create_typed_bind_group(texture)
    };

    (lut_texture, lut_texture_linear)
}
