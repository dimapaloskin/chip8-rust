struct ScreenSizeUniform {
    width: f32,
    height: f32,
};


@group(0) @binding(0)
var<uniform> u_size: ScreenSizeUniform;
@group(0) @binding(1)
var post_tex: texture_2d<f32>;
@group(0) @binding(2)
var post_sampler: sampler;
@group(0) @binding(3)
var<uniform> u_time: f32;
@group(0) @binding(4)
var<uniform> u_pp_enabled: u32;
@group(0) @binding(5)
var<uniform> u_sepia_amount: f32;

@vertex
fn vs_main(
    @builtin(vertex_index) vertexIndex : u32
) -> @builtin(position) vec4f {
    let pos = array(
        vec2f(-1.0,  1.0),  // top left
        vec2f( 1.0,  1.0),  // top right
        vec2f(-1.0, -1.0),  // bottom left

        vec2f( 1.0,  1.0),  // top right
        vec2f( 1.0, -1.0),  // bottom right
        vec2f(-1.0, -1.0)   // bottom left
    );

    return vec4f(pos[vertexIndex], 0.0, 1.0);
}

fn broken_display(col: vec4f, uv: vec2f, pos: vec4f) -> vec4f {
    let interval = 4.0;
    let m = pos.y / interval;
    let rem = m - floor(m / 2.0) * 2.0;
    let is_bright = floor(rem) == 0.0;
    let intensity = select(0.7, 1.0, is_bright);

    let flicker = 0.7 + 0.5 * sin(u_time * 1000.0 + pos.y * 14.1);

    return vec4f(col.rgb * intensity * flicker, col.a);
}

fn blurish(col: vec4f, uv: vec2f, pos: vec4f) -> vec4f {
    let r = 14.0 / u_size.width;
    var blur = vec3f(0.0, 0.0, 0.0);
    var size = 32;

    for(var i = 0; i < size; i = i + 1) {
        let angle = f32(i) * 6.2831853 / f32(size);
        let d = vec2f(cos(angle), sin(angle)) * r;
        blur += textureSample(post_tex, post_sampler, uv + d).rgb *
            step(0.4, max(max(textureSample(post_tex, post_sampler, uv + d).r,
                textureSample(post_tex, post_sampler, uv + d).g), textureSample(post_tex, post_sampler, uv + d).b));
    }

    blur /= f32(size);

    let glowed = col.rgb + blur * 1.0;
    return vec4f(clamp(glowed, vec3f(0.0), vec3f(1.0)), col.a);
}

fn soft_sepia(col: vec4f) -> vec4f {
    let r = dot(col.rgb, vec3f(0.393, 0.769, 0.189));
    let g = dot(col.rgb, vec3f(0.349, 0.686, 0.168));
    let b = dot(col.rgb, vec3f(0.272, 0.534, 0.131));

    let sepia = vec3f(r, g, b);

    let result = mix(col.rgb, sepia, u_sepia_amount);
    return vec4f(result, col.a);
}

@fragment
fn fs_main(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let uv = vec2f(pos.x / u_size.width, pos.y / u_size.height);
    let col = textureSample(post_tex, post_sampler, uv);

    if (u_pp_enabled == 0) {
        return soft_sepia(col);
    }

    let stage1 = blurish(col, uv, pos);
    let stage2 = soft_sepia(stage1);
    let stage3 = broken_display(stage2, uv, pos);

    return stage3;
}
