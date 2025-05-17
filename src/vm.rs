pub mod sprites;

use crate::video_buffer::{CHIP8_HEIGHT, CHIP8_WIDTH, VideoBuffer};

use sprites::SPRITES;

#[derive(Debug)]
pub struct Vm {
    pub vb: VideoBuffer,

    pc: u16,
    mem: [u8; 4096],

    reg: [u8; 16],
    ireg: u16,

    sp: u16,
    stack: [u16; 16],

    kb: [bool; 16],

    dt: u8,
    pub st: u8,

    rom_path: Option<String>,
}

impl Vm {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut vm = Self {
            vb: VideoBuffer::default(),

            pc: 0x200,
            mem: [0; 4096],

            reg: [0; 16],
            ireg: 0,

            sp: 0,
            stack: [0; 16],

            kb: [false; 16],

            dt: 0,
            st: 0,

            rom_path: None,
        };

        vm.mem[..SPRITES.len()].copy_from_slice(&SPRITES);

        vm
    }

    pub fn reset(&mut self) {
        self.pc = 0x200;
        self.mem = [0; 4096];
        self.mem[..SPRITES.len()].copy_from_slice(&SPRITES);
        self.reg = [0; 16];
        self.ireg = 0;
        self.sp = 0;
        self.stack = [0; 16];
        self.kb = [false; 16];
        self.dt = 0;
        self.st = 0;
    }

    pub fn load_rom(&mut self, rom_path: String) -> Result<(), std::io::Error> {
        let rom_bytes = std::fs::read(rom_path.clone())?;

        self.reset();

        self.rom_path = Some(rom_path);
        self.load_bin(&rom_bytes);
        Ok(())
    }

    fn load_bin(&mut self, rom: &[u8]) {
        self.mem[0x200..(0x200 + rom.len())].copy_from_slice(rom);
    }

    pub fn tick(&mut self) {
        let left = self.mem[self.pc as usize] as u16;
        let right = self.mem[(self.pc + 1) as usize] as u16;

        let opcode = (left << 8) | right;
        self.pc += 2;

        let a = (opcode >> 12) & 0x0f;
        let b = (opcode >> 8) & 0x0f;
        let c = (opcode >> 4) & 0x0f;
        let d = opcode & 0x0f;

        // http://devernay.free.fr/hacks/chip8/C8TECH10.HTM#3.1
        match (a, b, c, d) {
            /*
                00E0 - CLS
                Clear the display.
            */
            (0x00, 0x00, 0x0e, 0x00) => self.vb.clear(),

            /*
                00EE - RET
                Return from a subroutine.

                The interpreter sets the program counter to the address at the top of the stack, then subtracts 1 from the stack pointer.
            */
            (0x00, 0x00, 0x0e, 0x0e) => self.pc = self.pop(),

            /*
                1nnn - JP addr
                Jump to location nnn.

                The interpreter sets the program counter to nnn.
            */
            (0x01, _, _, _) => {
                let addr = opcode & 0x0fff;
                self.pc = addr;
            }

            /*
                2nnn - CALL addr
                Call subroutine at nnn.

                The interpreter increments the stack pointer, then puts the current PC on the top of the stack. The PC is then set to nnn.
            */
            (0x02, _, _, _) => {
                let addr = opcode & 0x0fff;
                self.push(self.pc);
                self.pc = addr;
            }

            /*
                3xkk - SE Vx, byte
                Skip next instruction if Vx = kk.

                The interpreter compares register Vx to kk, and if they are equal, increments the program counter by 2.
            */
            (0x03, _, _, _) => {
                let reg_val = self.reg[b as usize];
                let val = (opcode & 0xff) as u8;

                if reg_val == val {
                    self.pc += 2;
                }
            }

            /*
                4xkk - SNE Vx, byte
                Skip next instruction if Vx != kk.

                The interpreter compares register Vx to kk, and if they are not equal, increments the program counter by 2.
            */
            (0x04, _, _, _) => {
                let reg_val = self.reg[b as usize];
                let val = (opcode & 0xff) as u8;

                if reg_val != val {
                    self.pc += 2;
                }
            }

            /*
                5xy0 - SE Vx, Vy
                Skip next instruction if Vx = Vy.

                The interpreter compares register Vx to register Vy, and if they are equal, increments the program counter by 2.
            */
            (0x05, _, _, 0x00) => {
                let reg_x_val = self.reg[b as usize];
                let reg_y_val = self.reg[c as usize];

                if reg_x_val == reg_y_val {
                    self.pc += 2;
                }
            }

            /*
                6xkk - LD Vx, byte
                Set Vx = kk.

                The interpreter puts the value kk into register Vx.
            */
            (0x06, _, _, _) => {
                let val = (opcode & 0xff) as u8;
                self.reg[b as usize] = val;
            }

            /*
                7xkk - ADD Vx, byte
                Set Vx = Vx + kk.

                Adds the value kk to the value of register Vx, then stores the result in Vx.
            */
            (0x07, _, _, _) => {
                let val = (opcode & 0xff) as u8;
                let idx = b as usize;
                self.reg[idx] = self.reg[idx].wrapping_add(val);
            }

            /*
                8xy0 - LD Vx, Vy
                Set Vx = Vy.

                Stores the value of register Vy in register Vx.
            */
            (0x08, _, _, 0x00) => {
                let vy_val = self.reg[c as usize];
                self.reg[b as usize] = vy_val;
            }
            /*
                8xy1 - OR Vx, Vy
                Set Vx = Vx OR Vy.

                Performs a bitwise OR on the values of Vx and Vy, then stores the result in Vx.
                A bitwise OR compares the corrseponding bits from two values, and if either bit is 1,
                then the same bit in the result is also 1. Otherwise, it is 0.
            */
            (0x08, _, _, 0x01) => {
                self.reg[b as usize] |= self.reg[c as usize];
            }

            /*
                8xy2 - AND Vx, Vy
                Set Vx = Vx AND Vy.

                Performs a bitwise AND on the values of Vx and Vy, then stores the result in Vx.
                A bitwise AND compares the corrseponding bits from two values, and if both bits are 1,
                then the same bit in the result is also 1. Otherwise, it is 0.
            */
            (0x08, _, _, 0x02) => {
                self.reg[b as usize] &= self.reg[c as usize];
            }

            /*
                8xy3 - XOR Vx, Vy
                Set Vx = Vx XOR Vy.

                Performs a bitwise exclusive OR on the values of Vx and Vy, then stores the result in Vx.
                An exclusive OR compares the corrseponding bits from two values, and if the bits are not both the same,
                then the corresponding bit in the result is set to 1. Otherwise, it is 0.
            */
            (0x08, _, _, 0x03) => {
                self.reg[b as usize] ^= self.reg[c as usize];
            }

            /*
                8xy4 - ADD Vx, Vy
                Set Vx = Vx + Vy, set VF = carry.

                The values of Vx and Vy are added together. If the result is greater than 8 bits (i.e., > 255,) VF is set to 1, otherwise 0.
                Only the lowest 8 bits of the result are kept, and stored in Vx.
            */
            (0x08, _, _, 0x04) => {
                let (res, carry) = self.reg[b as usize].overflowing_add(self.reg[c as usize]);
                self.reg[b as usize] = res;
                self.reg[0x0f] = carry as u8;
            }

            /*
                8xy5 - SUB Vx, Vy
                Set Vx = Vx - Vy, set VF = NOT borrow.

                If Vx > Vy, then VF is set to 1, otherwise 0. Then Vy is subtracted from Vx, and the results stored in Vx.
            */
            (0x08, _, _, 0x05) => {
                let (res, carry) = self.reg[b as usize].overflowing_sub(self.reg[c as usize]);
                self.reg[b as usize] = res;
                self.reg[0x0f] = !carry as u8;
            }

            /*
                8xy6 - SHR Vx {, Vy}
                Set Vx = Vx SHR 1.

                If the least-significant bit of Vx is 1, then VF is set to 1, otherwise 0. Then Vx is divided by 2.
            */
            (0x08, _, _, 0x06) => {
                self.reg[0x0f] = self.reg[b as usize] & 0b1;
                self.reg[b as usize] >>= 1;
            }

            /*
                8xy7 - SUBN Vx, Vy
                Set Vx = Vy - Vx, set VF = NOT borrow.

                If Vy > Vx, then VF is set to 1, otherwise 0. Then Vx is subtracted from Vy, and the results stored in Vx.
            */
            (0x08, _, _, 0x07) => {
                let (new, carry) = self.reg[c as usize].overflowing_sub(self.reg[b as usize]);

                self.reg[b as usize] = new;
                self.reg[0x0f] = !carry as u8;
            }

            /*
                8xyE - SHL Vx {, Vy}
                Set Vx = Vx SHL 1.

                If the most-significant bit of Vx is 1, then VF is set to 1, otherwise to 0. Then Vx is multiplied by 2.
            */
            (0x08, _, _, 0x0e) => {
                self.reg[0x0f] = self.reg[b as usize] & 0b1;
                self.reg[0x0f] = (self.reg[b as usize] >> 7) & 1;
                self.reg[b as usize] <<= 1;
            }

            /*
                9xy0 - SNE Vx, Vy
                Skip next instruction if Vx != Vy.

                The values of Vx and Vy are compared, and if they are not equal, the program counter is increased by 2.
            */
            (0x09, _, _, 0x0) => {
                if self.reg[b as usize] != self.reg[c as usize] {
                    self.pc += 2;
                }
            }

            /*
                Annn - LD I, addr
                Set I = nnn.

                The value of register I is set to nnn.
            */
            (0x0a, _, _, _) => {
                self.ireg = opcode & 0x0fff;
            }

            /*
                Bnnn - JP V0, addr
                Jump to location nnn + V0.

                The program counter is set to nnn plus the value of V0.
            */
            (0x0b, _, _, _) => {
                let addr = (self.reg[0x00] as u16) + (opcode & 0x0fff);
                self.pc = addr;
            }

            /*
                Cxkk - RND Vx, byte
                Set Vx = random byte AND kk.

                The interpreter generates a random number from 0 to 255, which is then ANDed with the value kk.
                The results are stored in Vx. See instruction 8xy2 for more information on AND.
            */
            (0x0c, _, _, _) => {
                let rnd: u8 = rand::random();
                self.reg[b as usize] = rnd & (opcode & 0xff) as u8;
            }

            /*
                Dxyn - DRW Vx, Vy, nibble
                Display n-byte sprite starting at memory location I at (Vx, Vy), set VF = collision.

                The interpreter reads n bytes from memory, starting at the address stored in I.
                These bytes are then displayed as sprites on screen at coordinates (Vx, Vy).
                Sprites are XORed onto the existing screen.
                If this causes any pixels to be erased, VF is set to 1, otherwise it is set to 0.
                If the sprite is positioned so part of it is outside the coordinates of the display,
                it wraps around to the opposite side of the screen.
                See instruction 8xy3 for more information on XOR, and section 2.4,
                Display, for more information on the Chip-8 screen and sprites.
            */
            (0x0d, _, _, _) => {
                let x = self.reg[b as usize] as u16;
                let y = self.reg[c as usize] as u16;

                let n = d;

                let mut flipped = false;
                for sprite_y in 0..n {
                    let addr = self.ireg + sprite_y;
                    let pixels = self.mem[addr as usize];

                    for sprite_x in 0..8 {
                        let px = ((x + sprite_x) % CHIP8_WIDTH as u16) as usize;
                        let py = ((y + sprite_y) % CHIP8_HEIGHT as u16) as usize;

                        let sprite_bit = ((pixels >> (7 - sprite_x)) & 1) != 0;
                        let was_on = self.vb.xor_pixel(px, py, sprite_bit);
                        if was_on && sprite_bit {
                            flipped = true;
                        }
                    }
                }

                self.reg[0xf] = flipped as u8;
            }

            /*
                Ex9E - SKP Vx
                Skip next instruction if key with the value of Vx is pressed.

                Checks the keyboard, and if the key corresponding to the value of Vx is currently in the down position, PC is increased by 2.
            */
            (0x0e, _, 0x09, 0x0e) => {
                let key_num = self.reg[b as usize];
                if self.kb[key_num as usize] {
                    self.pc += 2;
                }
            }

            /*
                ExA1 - SKNP Vx
                Skip next instruction if key with the value of Vx is not pressed.

                Checks the keyboard, and if the key corresponding to the value of Vx is currently in the up position, PC is increased by 2.
            */
            (0x0e, _, 0x0a, 0x01) => {
                let key_num = self.reg[b as usize];
                if !self.kb[key_num as usize] {
                    self.pc += 2;
                }
            }

            /*
                Fx07 - LD Vx, DT
                Set Vx = delay timer value.

                The value of DT is placed into Vx.
            */
            (0x0f, _, 0x00, 0x07) => {
                self.reg[b as usize] = self.dt;
            }

            /*
                Fx0A - LD Vx, K
                Wait for a key press, store the value of the key in Vx.

                All execution stops until a key is pressed, then the value of that key is stored in Vx.
            */
            (0x0f, _, 0x00, 0x0a) => {
                if let Some(key) = self.kb.iter().position(|&x| x) {
                    self.reg[b as usize] = key as u8;
                } else {
                    self.pc -= 2;
                }
            }
            /*
                Fx15 - LD DT, Vx
                Set delay timer = Vx.

                DT is set equal to the value of Vx.
            */
            (0x0f, _, 0x01, 0x05) => {
                self.dt = self.reg[b as usize];
            }

            /*
                Fx18 - LD ST, Vx
                Set sound timer = Vx.

                ST is set equal to the value of Vx.
            */
            (0x0f, _, 0x01, 0x08) => {
                self.st = self.reg[b as usize];
            }

            /*
                Fx1E - ADD I, Vx
                Set I = I + Vx.

                The values of I and Vx are added, and the results are stored in I.
            */
            (0x0f, _, 0x01, 0x0e) => {
                let reg_val = self.reg[b as usize] as u16;
                self.ireg = self.ireg.wrapping_add(reg_val)
            }

            /*
                Fx29 - LD F, Vx
                Set I = location of sprite for digit Vx.

                The value of I is set to the location for the hexadecimal sprite corresponding to the value of Vx.
                See section 2.4, Display, for more information on the Chip-8 hexadecimal font.
            */
            (0x0f, _, 0x02, 0x09) => {
                let num = self.reg[b as usize];
                self.ireg = num as u16 * 5;
            }

            /*
                Fx33 - LD B, Vx
                Store BCD representation of Vx in memory locations I, I+1, and I+2.

                The interpreter takes the decimal value of Vx, and places the hundreds digit in memory at location in I,
                the tens digit at location I+1, and the ones digit at location I+2.
            */
            (0x0f, _, 0x03, 0x03) => {
                let reg_val = self.reg[b as usize];

                let d100 = reg_val / 100;
                let d10 = (reg_val % 100) / 10;
                let d1 = reg_val % 10;

                self.mem[self.ireg as usize] = d100;
                self.mem[(self.ireg + 1) as usize] = d10;
                self.mem[(self.ireg + 2) as usize] = d1;
            }

            /*
                Fx55 - LD [I], Vx
                Store registers V0 through Vx in memory starting at location I.

                The interpreter copies the values of registers V0 through Vx into memory, starting at the address in I.
            */
            (0x0f, _, 0x05, 0x05) => {
                for idx in 0..=(b as usize) {
                    self.mem[(self.ireg + idx as u16) as usize] = self.reg[idx];
                }
            }

            /*
                Fx65 - LD Vx, [I]
                Read registers V0 through Vx from memory starting at location I.

                The interpreter reads values from memory starting at location I into registers V0 through Vx.
            */
            (0x0f, _, 0x06, 0x05) => {
                for idx in 0..=(b as usize) {
                    self.reg[idx] = self.mem[(self.ireg + idx as u16) as usize];
                }
            }
            _ => {
                panic!("Unknown opcode: 0x{:x}", opcode)
            }
        }
    }

    // once per frame, at rate 60Hz
    pub fn delay_timer(&mut self) {
        if self.dt > 0 {
            self.dt -= 1;
        }
    }

    // once per frame, at rate 60Hz
    pub fn sound_timer(&mut self) {
        if self.st > 0 {
            self.st -= 1;
        }
    }

    fn push(&mut self, val: u16) {
        self.stack[self.sp as usize] = val;
        self.sp += 1;
    }

    fn pop(&mut self) -> u16 {
        self.sp -= 1;
        self.stack[self.sp as usize]
    }

    pub fn set_kb(&mut self, key: usize, state: bool) {
        self.kb[key] = state;
    }
}
