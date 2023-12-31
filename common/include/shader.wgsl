struct Locals {
	screen_size: vec2<f32>,
	texture_size: u32,
	_padding0: u32,
	_padding1: vec2<u32>,
	_padding2: vec2<u32>,
}

@group(0) @binding(0) var<uniform> u_locals: Locals;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var tex_s: sampler;

fn unpack_color(color: u32) -> vec4<f32> {
    return vec4(
        f32((color >> 24u) & 255u),
        f32((color >> 16u) & 255u),
        f32((color >> 8u) & 255u),
        f32(color & 255u),
    ) / 255.0;
}

fn position_from_screen(screen_pos: vec2<f32>) -> vec4<f32> {
    return vec4(
        2.0 * screen_pos.x / u_locals.screen_size.x - 1.0,
        1.0 - 2.0 * screen_pos.y / u_locals.screen_size.y,
        0.0,
        1.0,
    );
}

struct Vertex {
	@location(0) pos: vec2<f32>,
	@location(1) uv: vec2<u32>,
	@location(2) color: u32,
}

struct Fragment {
	@builtin(position) pos: vec4<f32>,
	@location(0) uv: vec2<f32>,
	@location(1) color: vec4<f32>,
}

@vertex
fn vs_main(in: Vertex) -> Fragment {
	var out: Fragment;
	out.pos = position_from_screen(in.pos);

	out.uv = vec2<f32>(in.uv) / f32(u_locals.texture_size);
	out.color = unpack_color(in.color);
	return out;
}

@fragment
fn fs_main(in: Fragment) -> @location(0) vec4<f32> {
	return textureSample(tex, tex_s, in.uv) * in.color;
}
