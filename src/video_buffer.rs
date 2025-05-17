pub const CHIP8_WIDTH: usize = 64;
pub const CHIP8_HEIGHT: usize = 32;

const BUFFER_WORDS: usize = CHIP8_WIDTH * CHIP8_HEIGHT / 32;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VideoBuffer {
    pixels: [u32; BUFFER_WORDS],
}

impl Default for VideoBuffer {
    fn default() -> Self {
        Self {
            pixels: [0; BUFFER_WORDS],
        }
    }
}

impl VideoBuffer {
    pub fn set_pixel(&mut self, x: usize, y: usize, value: bool) {
        let idx = y * CHIP8_WIDTH + x;
        let word = idx / 32;
        let bit = idx % 32;
        if value {
            self.pixels[word] |= 1 << bit;
        } else {
            self.pixels[word] &= !(1 << bit);
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        let idx = y * CHIP8_WIDTH + x;
        let word = idx / 32;
        let bit = idx % 32;
        (self.pixels[word] >> bit) & 1 != 0
    }

    pub fn toggle_pixel(&mut self, x: usize, y: usize) {
        let idx = y * CHIP8_WIDTH + x;
        let word = idx / 32;
        let bit = idx % 32;
        self.pixels[word] ^= 1 << bit;
    }

    pub fn xor_pixel(&mut self, x: usize, y: usize, value: bool) -> bool {
        let idx = y * CHIP8_WIDTH + x;
        let word = idx / 32;
        let bit = idx % 32;
        let current = (self.pixels[word] >> bit) & 1 != 0;
        self.pixels[word] ^= (value as u32) << bit;
        current
    }

    pub fn clear(&mut self) {
        self.pixels = [0; BUFFER_WORDS];
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_pixel() {
        let mut buffer = VideoBuffer::default();
        buffer.set_pixel(0, 0, true);
        assert!(buffer.get_pixel(0, 0));
        assert!(!buffer.get_pixel(1, 0));
    }

    #[test]
    fn test_all_pixels() {
        let mut buffer = VideoBuffer::default();

        for x in 0..CHIP8_WIDTH {
            for y in 0..CHIP8_HEIGHT {
                buffer.set_pixel(x, y, true);
                assert!(buffer.get_pixel(x, y));
            }
        }

        for x in 0..CHIP8_WIDTH {
            for y in 0..CHIP8_HEIGHT {
                buffer.toggle_pixel(x, y);
                assert!(!buffer.get_pixel(x, y));
            }
        }
    }
}
