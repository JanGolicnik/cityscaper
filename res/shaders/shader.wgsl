struct Camera {
    up: vec4<f32>,
    right: vec4<f32>,
    position: vec4<f32>,
    direction: vec4<f32>,
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct RenderData {
    time: f32,
    wind_strength: f32,
    wind_scale: f32,
    wind_speed: f32,
    wind_direction: f32,
    wind_noise_scale: f32,
    wind_noise_strength: f32,
    padding: f32
};

@group(1) @binding(0)
var<uniform> render_data: RenderData;

@group(2) @binding(0)
var tex: texture_2d<f32>;
@group(2) @binding(1)
var tex_sampler: sampler;

@group(3) @binding(0)
var lut_tex: texture_2d<f32>;
@group(3) @binding(1)
var lut_tex_sampler: sampler;

struct VertexInput{
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) age: f32,
};

struct InstanceInput{
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,

    @location(9)  inv_model_matrix_0: vec4<f32>,
    @location(10) inv_model_matrix_1: vec4<f32>,
    @location(11) inv_model_matrix_2: vec4<f32>,
    @location(12) inv_model_matrix_3: vec4<f32>,
}

struct VertexOutput{
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) age: f32,
    @location(2) world_pos: vec3<f32>,
    @location(3) scale: vec3<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput
) -> VertexOutput{

    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let inv_model_matrix = mat4x4<f32>(
        instance.inv_model_matrix_0,
        instance.inv_model_matrix_1,
        instance.inv_model_matrix_2,
        instance.inv_model_matrix_3,
    );

    var world_position = model_matrix * vec4<f32>(model.position, 1.0);
    let normal = transpose(inv_model_matrix) * vec4<f32>(model.normal, 1.0);

    let scale1 = length(vec3<f32>(instance.model_matrix_0.x, instance.model_matrix_1.x, instance.model_matrix_2.x));
    let scale2 = length(vec3<f32>(instance.model_matrix_0.y, instance.model_matrix_1.y, instance.model_matrix_2.y));
    let scale3 = length(vec3<f32>(instance.model_matrix_0.z, instance.model_matrix_1.z, instance.model_matrix_2.z));
    let scale = vec3<f32>(scale1, scale2, scale3);

    let wind = calculate_wind(world_position.xz);
    let t = min(world_position.y / 0.1, 1.0);
    let age = pow(min(model.age, 1.0), 2.0);
    world_position.z += wind * age * t;
    
    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_position;
    out.normal = normalize(normal.xyz);
    out.age = model.age;
    out.world_pos = world_position.xyz;
    out.scale = scale;
    
    return out;
}

fn sample_ground(world_pos: vec3<f32>) -> vec3<f32> {
    let uv = world_pos * 0.1;
    let ground = textureSample(tex, tex_sampler, uv.xz).r * 0.01;
    let lut = textureSample(lut_tex, lut_tex_sampler, vec2<f32>(0.0, 0.5)).rgb;
    return lut * (1.0 - ground);
}

fn get_shadow(normal: vec3<f32>) -> f32{
    let light_dir = vec3<f32>(-1.0);

    let d = max(dot(light_dir, normal), 0.0);
    return 1.0 - d * 0.05;
}

@fragment
fn fs_color_object(in: VertexOutput) -> @location(0) vec4<f32>{
    var ground = sample_ground(in.world_pos);    
    var t = clamp(in.world_pos.y / 0.1, 0.0, 1.0);

    let lut = textureSample(lut_tex, lut_tex_sampler, vec2<f32>(in.age, 0.5)).rgb;

    let color = lut * t * get_shadow(in.normal) + vec3<f32>(ground * (1.0 - t));

    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_dust(in: VertexOutput) -> @location(0) vec4<f32>{
    let t = 1.0 - in.scale.x / 0.01;
    let color = textureSample(lut_tex, lut_tex_sampler, vec2<f32>(t, 0.5)).rgb;
    // return vec4<f32>(vec3<f32>(in.scale.x/ 0.0085), 1.0);
    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_floor(in: VertexOutput) -> @location(0) vec4<f32>{
    // let wind = calculate_wind(in.world_pos.xz);
    // return vec4<f32>(vec3<f32>(wind), 1.0);
    return vec4<f32>(sample_ground(in.world_pos), 1.0);
}

@fragment
fn fs_grass(in: VertexOutput) -> @location(0) vec4<f32>{
    let ground = sample_ground(in.world_pos);
    
    var t = min(in.world_pos.y / 0.1, 1.0);
    let color = textureSample(lut_tex, lut_tex_sampler, vec2<f32>(t, 0.5)).rgb;

    // return vec4<f32>(vec3<f32>(t), 1.0);
    return vec4<f32>(color, 1.0);
}

fn calculate_wind(coords: vec2<f32>) -> f32{
    let time = render_data.time * render_data.wind_speed;

    let noise = textureSampleLevel(tex, tex_sampler, coords * render_data.wind_noise_scale + time * 0.01, 0.0).r;

    var pos = (coords.x + coords.y) + noise * render_data.wind_noise_strength;
    return sin(pos * render_data.wind_scale + time) * render_data.wind_strength;
}

