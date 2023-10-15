struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) hasTexture: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    if (model.hasTexture == 1.0) {
        return out;
    } else {
        out.color = model.color;
        out.clip_position = vec4<f32>(model.pos.x, model.pos.y, 0.0, 1.0);
        return out;
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}