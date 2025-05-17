struct ScreenSizeUniform {
    width: f32,
    height: f32,
};

struct VideoBuffer {
    pixels: array<u32, 64>,
};

@group(0) @binding(0)
var<uniform> u_fg_color: vec4<f32>;
@group(0) @binding(1)
var<uniform> u_bg_color: vec4<f32>;
@group(0) @binding(2)
var<uniform> u_size: ScreenSizeUniform;

@group(1) @binding(0)
var<storage, read> video: VideoBuffer;

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

@fragment
fn fs_main(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let CHIP8_WIDTH: f32 = 64.0;
    let CHIP8_HEIGHT: f32 = 32.0;

    let px = pos.x;
    let py = pos.y;

    let w = u_size.width;
    let h = u_size.height;

    let chip_x = u32(clamp(floor(px * CHIP8_WIDTH / w), 0.0, CHIP8_WIDTH - 1.0));
    let chip_y = u32(clamp(floor(py * CHIP8_HEIGHT / h), 0.0, CHIP8_HEIGHT - 1.0));

    let idx = chip_y * u32(CHIP8_WIDTH) + chip_x;
    let word = idx / 32u;
    let bit = idx % 32u;
    let pix = (video.pixels[word] >> bit) & 1u;

    if pix == 1u {
        return u_fg_color;
    } else {
        return u_bg_color;
    }
}
