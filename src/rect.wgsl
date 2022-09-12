// Vertex shader

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) vert_color : vec3<f32>
};

struct VertexInput {
    @location(0) position : vec2<f32>,
    @location(1) color: vec3<f32>
}

@vertex
fn vs_main(
    model : VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);
    out.vert_color = model.color;
    return out;
}

 // Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.vert_color, 1.0);
}