use std::collections::HashMap;

use image::GenericImageView;
use jandering_engine::{
    core::{
        bind_group::{
            camera::free::{CameraController, MatrixCameraBindGroup},
            texture::TextureBindGroup,
        },
        object::{Instance, Object, Vertex},
        renderer::{
            create_typed_bind_group, create_typed_bind_group_at, get_typed_bind_group,
            BindGroupHandle, Renderer, SamplerHandle, ShaderHandle, TextureHandle,
        },
        shader::ShaderDescriptor,
        texture::{sampler::SamplerDescriptor, TextureDescriptor, TextureFormat},
    },
    types::{UVec2, Vec2, Vec3},
    utils::load_text,
};

use crate::{
    camera_controller::IsometricCameraController,
    color_obj::{AgeObject, AgeVertex},
    image::Image,
};

use super::{
    logic::read_lut, Plants, RenderDataBindGroup, N_DUST, N_GRASS, ORTHO_FAR, ORTHO_HEIGHT,
    ORTHO_NEAR, ORTHO_WIDTH, REFERENCE_DIAGONAL,
};

pub fn create_camera(renderer: &mut dyn Renderer) -> BindGroupHandle<MatrixCameraBindGroup> {
    let (aspect, diagonal) = {
        let size = renderer.size();
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
    create_typed_bind_group(renderer, camera)
}

pub fn create_objects(
    renderer: &mut dyn Renderer,
) -> (Plants, Object<Instance>, AgeObject, AgeObject) {
    let floor = Object::quad(
        renderer,
        vec![Instance::default()
            .rotate(90.0f32.to_radians(), Vec3::X)
            .set_size(Vec3::splat(100.0))],
    );

    let mut plants = HashMap::new();
    plants.reserve(50);

    let dust_instances = (0..N_DUST)
        .map(|_| Instance::default().translate(Vec3::splat(-1000.0)))
        .collect();
    let dust = AgeObject::quad(renderer, 0.3, dust_instances);

    let grass_instances = (0..N_GRASS)
        .map(|_| {
            Instance::default()
                .set_size(Vec3::new(0.008, 0.1, 1.0))
                .set_position(Vec3::new(1000.0, 0.0, 0.0))
        })
        .collect::<Vec<_>>();
    let grass = AgeObject::quad(renderer, 1.0, grass_instances);

    (plants, floor, dust, grass)
}

pub async fn create_textures(
    renderer: &mut dyn Renderer,
) -> (
    TextureHandle,
    Image,
    BindGroupHandle<TextureBindGroup>,
    SamplerHandle,
    BindGroupHandle<TextureBindGroup>,
    BindGroupHandle<TextureBindGroup>,
) {
    let (lut_texture, lut_texture_linear, lut_sampler) =
        create_lut_textures(renderer, None, None, None);
    let depth_texture = renderer.create_texture(TextureDescriptor {
        size: renderer.size(),
        format: TextureFormat::Depth32F,
        ..Default::default()
    });
    let noise_image = image::load_from_memory(include_bytes!("../../res/noise.png")).unwrap();
    let noise_texture = {
        let tex_sampler = renderer.create_sampler(SamplerDescriptor {
            address_mode: jandering_engine::core::texture::sampler::SamplerAddressMode::Repeat,
            ..Default::default()
        });
        let noise_handle = renderer.create_texture(TextureDescriptor {
            data: Some(&noise_image.to_rgba8()),
            size: noise_image.dimensions().into(),
            format: TextureFormat::Rgba8U,
            ..Default::default()
        });
        let noise_texture = TextureBindGroup::new(renderer, noise_handle, tex_sampler);
        create_typed_bind_group(renderer, noise_texture)
    };
    let noise_image = Image::new(noise_image.to_rgb32f(), 0.1);

    (
        depth_texture,
        noise_image,
        noise_texture,
        lut_sampler,
        lut_texture,
        lut_texture_linear,
    )
}

pub async fn create_shaders(
    renderer: &mut dyn Renderer,
) -> (ShaderHandle, ShaderHandle, ShaderHandle, ShaderHandle) {
    let descriptor = ShaderDescriptor::default()
        .with_source(jandering_engine::core::shader::ShaderSource::Code(
            load_text(jandering_engine::utils::FilePath::FileName(
                "shaders/shader.wgsl",
            ))
            .await
            .unwrap(),
        ))
        .with_descriptors(vec![AgeVertex::desc(), Instance::desc()])
        .with_depth(true)
        .with_backface_culling(false)
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
    renderer: &mut dyn Renderer,
    lut_handle: Option<BindGroupHandle<TextureBindGroup>>,
    lut_handle_linear: Option<BindGroupHandle<TextureBindGroup>>,
    mut lut_sampler: Option<SamplerHandle>,
) -> (
    BindGroupHandle<TextureBindGroup>,
    BindGroupHandle<TextureBindGroup>,
    SamplerHandle,
) {
    if lut_sampler.is_none() {
        lut_sampler = Some(renderer.create_sampler(SamplerDescriptor {
            address_mode: jandering_engine::core::texture::sampler::SamplerAddressMode::Clamp,
            ..Default::default()
        }));
    }

    let data = read_lut(false)
        .unwrap_or_default()
        .iter()
        .take(renderer.max_texture_size().x as usize)
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
        let texture_handle = get_typed_bind_group(renderer, handle)
            .unwrap()
            .texture_handle;
        renderer.re_create_texture(desc.clone(), texture_handle);
        let texture = TextureBindGroup::new(renderer, texture_handle, lut_sampler.unwrap());

        create_typed_bind_group_at(renderer, texture, handle);
        handle
    } else {
        let handle = renderer.create_texture(desc.clone());
        let texture = TextureBindGroup::new(renderer, handle, lut_sampler.unwrap());
        create_typed_bind_group(renderer, texture)
    };

    let data = read_lut(true)
        .unwrap_or_default()
        .iter()
        .take(renderer.max_texture_size().x as usize)
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
        let texture_handle = get_typed_bind_group(renderer, handle)
            .unwrap()
            .texture_handle;
        renderer.re_create_texture(desc, texture_handle);
        let texture = TextureBindGroup::new(renderer, texture_handle, lut_sampler.unwrap());

        create_typed_bind_group_at(renderer, texture, handle);
        handle
    } else {
        let handle = renderer.create_texture(desc);
        let texture = TextureBindGroup::new(renderer, handle, lut_sampler.unwrap());
        create_typed_bind_group(renderer, texture)
    };

    (lut_texture, lut_texture_linear, lut_sampler.unwrap())
}
