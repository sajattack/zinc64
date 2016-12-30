/*
 * Copyright (c) 2016 DigitalStream <https://www.digitalstream.io>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::cell::RefCell;
use std::rc::Rc;

use config::Config;
use cpu::Cpu;
use mem::{Addressable, Memory};
use video::{ColorRam, RenderTarget};
use util::bit;

// SPEC: The MOS 6567/6569 video controller (VIC-II) and its application in the Commodore 64

// TODO vic: implement raster int
// TODO vic: implement rsel/csel
// TODO vic: implement scroll
// TODO vic: implement remaining modes
// TODO vic: implement sprites

pub struct Vic {
    // Dependencies
    config: Config,
    cpu: Rc<RefCell<Cpu>>,
    mem: Rc<RefCell<Memory>>,
    color_ram: Rc<RefCell<ColorRam>>,
    rt: Rc<RefCell<RenderTarget>>,
    // Control
    mode: Mode,
    enabled: bool,
    rsel: bool,
    csel: bool,
    scroll_x: u8,
    scroll_y: u8,
    irq_enable: u8,
    irq_status: u8,
    // Internal Counters
    raster: u16,
    raster_compare: u16,
    video_counter: u16,
    // Memory Pointers
    char_base: u16,
    video_matrix: u16,
    // Color and Sprite Data
    border_color: u8,
    background_color: [u8; 4],
    sprites: [Sprite; 8],
    sprite_multicolor: [u8; 2],
    // Misc
    light_pen_pos: [u8; 2],
}

#[derive(Copy, Clone)]
enum Mode {
    // (ECM/BMM/MCM=0/0/0)
    Text = 0x00,
    // (ECM/BMM/MCM=0/0/1)
    McText = 0x01,
    // (ECM/BMM/MCM=0/1/0)
    Bitmap = 0x02,
    // (ECM/BMM/MCM=0/1/1)
    McBitmap = 0x03,
    // (ECM/BMM/MCM=1/0/0)
    EcmText = 0x04,
    // (ECM/BMM/MCM=1/0/1)
    InvalidText = 0x05,
    // (ECM/BMM/MCM=1/1/0)
    InvalidBitmap1 = 0x06,
    // (ECM/BMM/MCM=1/1/1)
    InvalidBitmap2 = 0x07,
}

impl Mode {
    pub fn from(mode: u8) -> Mode {
        match mode {
            0x00 => Mode::Text,
            0x01 => Mode::McText,
            0x02 => Mode::Bitmap,
            0x03 => Mode::McBitmap,
            0x04 => Mode::EcmText,
            0x05 => Mode::InvalidText,
            0x06 => Mode::InvalidBitmap1,
            0x07 => Mode::InvalidBitmap2,
            _ => panic!("invalid mode {}", mode),
        }
    }

    pub fn value(&self) -> u8 {
        *self as u8
    }
}

#[derive(Copy, Clone)]
pub enum Reg {
    M0X = 0x00,
    M0Y = 0x01,
    M1X = 0x02,
    M1Y = 0x03,
    M2X = 0x04,
    M2Y = 0x05,
    M3X = 0x06,
    M3Y = 0x07,
    M4X = 0x08,
    M4Y = 0x09,
    M5X = 0x0a,
    M5Y = 0x0b,
    M6X = 0x0c,
    M6Y = 0x0d,
    M7X = 0x0e,
    M7Y = 0x0f,
    MX8 = 0x10,
    CR1 = 0x11,
    RASTER = 0x12,
    LPX = 0x13,
    LPY = 0x14,
    ME = 0x15,
    CR2 = 0x16,
    MYE = 0x17,
    MEMPTR = 0x18,
    IRR = 0x19,
    IMR = 0x1a,
    MDP = 0x1b,
    MMC = 0x1c,
    MXE = 0x1d,
    MM = 0x1e,
    MD = 0x1f,
    EC = 0x20,
    B0C = 0x21,
    B1C = 0x22,
    B2C = 0x23,
    B3C = 0x24,
    MM0 = 0x25,
    MM1 = 0x26,
    M0C = 0x27,
    M1C = 0x28,
    M2C = 0x29,
    M3C = 0x2a,
    M4C = 0x2b,
    M5C = 0x2c,
    M6C = 0x2d,
    M7C = 0x2e,
    IGNORE = 0xff,
}

impl Reg {
    pub fn from(reg: u8) -> Reg {
        match reg {
            0x00 => Reg::M0X,
            0x01 => Reg::M0Y,
            0x02 => Reg::M1X,
            0x03 => Reg::M1Y,
            0x04 => Reg::M2X,
            0x05 => Reg::M2Y,
            0x06 => Reg::M3X,
            0x07 => Reg::M3Y,
            0x08 => Reg::M4X,
            0x09 => Reg::M4Y,
            0x0a => Reg::M5X,
            0x0b => Reg::M5Y,
            0x0c => Reg::M6X,
            0x0d => Reg::M6Y,
            0x0e => Reg::M7X,
            0x0f => Reg::M7Y,
            0x10 => Reg::MX8,
            0x11 => Reg::CR1,
            0x12 => Reg::RASTER,
            0x13 => Reg::LPX,
            0x14 => Reg::LPY,
            0x15 => Reg::ME,
            0x16 => Reg::CR2,
            0x17 => Reg::MYE,
            0x18 => Reg::MEMPTR,
            0x19 => Reg::IRR,
            0x1a => Reg::IMR,
            0x1b => Reg::MDP,
            0x1c => Reg::MMC,
            0x1d => Reg::MXE,
            0x1e => Reg::MM,
            0x1f => Reg::MD,
            0x20 => Reg::EC,
            0x21 => Reg::B0C,
            0x22 => Reg::B1C,
            0x23 => Reg::B2C,
            0x24 => Reg::B3C,
            0x25 => Reg::MM0,
            0x26 => Reg::MM1,
            0x27 => Reg::M0C,
            0x28 => Reg::M1C,
            0x29 => Reg::M2C,
            0x2a => Reg::M3C,
            0x2b => Reg::M4C,
            0x2c => Reg::M5C,
            0x2d => Reg::M6C,
            0x2e => Reg::M7C,
            0x2f ... 0x3f => Reg::IGNORE,
            _ => panic!("invalid reg {}", reg),
        }
    }
}

#[derive(Copy, Clone)]
struct Sprite {
    enabled: bool,
    x: u16,
    y: u8,
    color: u8,
    expand_x: bool,
    expand_y: bool,
    multicolor: bool,
    priority: bool,
}

impl Sprite {
    pub fn new() -> Sprite {
        Sprite {
            enabled: false,
            x: 0,
            y: 0,
            color: 0,
            expand_x: false,
            expand_y: false,
            multicolor: false,
            priority: true,
        }
    }
}

impl Vic {
    pub fn new(config: Config,
               cpu: Rc<RefCell<Cpu>>,
               mem: Rc<RefCell<Memory>>,
               color_ram: Rc<RefCell<ColorRam>>,
               rt: Rc<RefCell<RenderTarget>>) -> Vic {
        Vic {
            config: config,
            cpu: cpu,
            mem: mem,
            color_ram: color_ram,
            rt: rt,
            mode: Mode::Text,
            enabled: true,
            rsel: true,
            csel: true,
            scroll_x: 0,
            scroll_y: 3,
            irq_enable: 0x00,
            irq_status: 0x00,
            raster: 0x0100,
            raster_compare: 0x00,
            video_counter: 0,
            char_base: 4096,
            video_matrix: 1024,
            border_color: 0x0e,
            background_color: [0x06, 0, 0, 0],
            sprites: [Sprite::new(); 8],
            sprite_multicolor: [0; 2],
            light_pen_pos: [0; 2],
        }
    }

    // -- Device I/O

    pub fn read(&mut self, reg: u8) -> u8 {
        match Reg::from(reg) {
            Reg::M0X => (self.sprites[0].x & 0x00ff) as u8,
            Reg::M0Y => self.sprites[0].y,
            Reg::M1X => (self.sprites[1].x & 0x00ff) as u8,
            Reg::M1Y => self.sprites[1].y,
            Reg::M2X => (self.sprites[2].x & 0x00ff) as u8,
            Reg::M2Y => self.sprites[2].y,
            Reg::M3X => (self.sprites[3].x & 0x00ff) as u8,
            Reg::M3Y => self.sprites[3].y,
            Reg::M4X => (self.sprites[4].x & 0x00ff) as u8,
            Reg::M4Y => self.sprites[4].y,
            Reg::M5X => (self.sprites[5].x & 0x00ff) as u8,
            Reg::M5Y => self.sprites[5].y,
            Reg::M6X => (self.sprites[6].x & 0x00ff) as u8,
            Reg::M6Y => self.sprites[6].y,
            Reg::M7X => (self.sprites[7].x & 0x00ff) as u8,
            Reg::M7Y => self.sprites[7].y,
            Reg::MX8 => {
                let m0x8 = bit::bit_val16(self.sprites[0].x, 8) << 0;
                let m1x8 = bit::bit_val16(self.sprites[1].x, 8) << 1;
                let m2x8 = bit::bit_val16(self.sprites[2].x, 8) << 2;
                let m3x8 = bit::bit_val16(self.sprites[3].x, 8) << 3;
                let m4x8 = bit::bit_val16(self.sprites[4].x, 8) << 4;
                let m5x8 = bit::bit_val16(self.sprites[5].x, 8) << 5;
                let m6x8 = bit::bit_val16(self.sprites[6].x, 8) << 6;
                let m7x8 = bit::bit_val16(self.sprites[7].x, 8) << 7;
                m0x8 | m1x8 | m2x8 | m3x8 | m4x8 | m5x8 | m6x8 | m7x8
            },
            Reg::CR1 => {
                let rst8 = bit::bit_val16(self.raster, 8) << 7;
                let ecm = bit::bit_val(self.mode.value(), 2) << 6;
                let bmm = bit::bit_val(self.mode.value(), 1) << 5;
                let den = bit::bit_set(4, self.enabled);
                let rsel = bit::bit_set(3, self.rsel);
                let yscroll = self.scroll_y & 0x07;
                rst8 | ecm | bmm | den | rsel | yscroll
            }
            Reg::RASTER => (self.raster & 0x00ff) as u8,
            Reg::LPX => self.light_pen_pos[0],
            Reg::LPY => self.light_pen_pos[1],
            Reg::ME => {
                let m0e = bit::bit_set(0, self.sprites[0].enabled);
                let m1e = bit::bit_set(1, self.sprites[1].enabled);
                let m2e = bit::bit_set(2, self.sprites[2].enabled);
                let m3e = bit::bit_set(3, self.sprites[3].enabled);
                let m4e = bit::bit_set(4, self.sprites[4].enabled);
                let m5e = bit::bit_set(5, self.sprites[5].enabled);
                let m6e = bit::bit_set(6, self.sprites[6].enabled);
                let m7e = bit::bit_set(7, self.sprites[7].enabled);
                m0e | m1e | m2e | m3e | m4e | m5e | m6e | m7e
            },
            Reg::CR2 => {
                let res = 1 << 5;
                let mcm = bit::bit_val(self.mode.value(), 0) << 4;
                let csel = bit::bit_set(3, self.csel);
                let yscroll = self.scroll_x & 0x07;
                res | mcm | csel | yscroll | 0xc0
            }
            Reg::MYE => {
                let m0ye = bit::bit_set(0, self.sprites[0].expand_y);
                let m1ye = bit::bit_set(1, self.sprites[1].expand_y);
                let m2ye = bit::bit_set(2, self.sprites[2].expand_y);
                let m3ye = bit::bit_set(3, self.sprites[3].expand_y);
                let m4ye = bit::bit_set(4, self.sprites[4].expand_y);
                let m5ye = bit::bit_set(5, self.sprites[5].expand_y);
                let m6ye = bit::bit_set(6, self.sprites[6].expand_y);
                let m7ye = bit::bit_set(7, self.sprites[7].expand_y);
                m0ye | m1ye | m2ye | m3ye | m4ye | m5ye | m6ye | m7ye
            },
            Reg::MEMPTR => {
                let vm = ((self.video_matrix & 0x3c00 >> 10) as u8) << 4;
                let cb = ((self.char_base & 0x3800 >> 11) as u8) << 1;
                vm | cb | 0x01
            },
            Reg::IRR => self.irq_status,
            Reg::IMR => self.irq_enable,
            Reg::MDP => {
                let m0dp = bit::bit_set(0, self.sprites[0].priority);
                let m1dp = bit::bit_set(1, self.sprites[1].priority);
                let m2dp = bit::bit_set(2, self.sprites[2].priority);
                let m3dp = bit::bit_set(3, self.sprites[3].priority);
                let m4dp = bit::bit_set(4, self.sprites[4].priority);
                let m5dp = bit::bit_set(5, self.sprites[5].priority);
                let m6dp = bit::bit_set(6, self.sprites[6].priority);
                let m7dp = bit::bit_set(7, self.sprites[7].priority);
                m0dp | m1dp | m2dp | m3dp | m4dp | m5dp | m6dp | m7dp
            },
            Reg::MMC => {
                let m0mc = bit::bit_set(0, self.sprites[0].multicolor);
                let m1mc = bit::bit_set(1, self.sprites[1].multicolor);
                let m2mc = bit::bit_set(2, self.sprites[2].multicolor);
                let m3mc = bit::bit_set(3, self.sprites[3].multicolor);
                let m4mc = bit::bit_set(4, self.sprites[4].multicolor);
                let m5mc = bit::bit_set(5, self.sprites[5].multicolor);
                let m6mc = bit::bit_set(6, self.sprites[6].multicolor);
                let m7mc = bit::bit_set(7, self.sprites[7].multicolor);
                m0mc | m1mc | m2mc | m3mc | m4mc | m5mc | m6mc | m7mc
            },
            Reg::MXE => {
                let m0xe = bit::bit_set(0, self.sprites[0].expand_x);
                let m1xe = bit::bit_set(1, self.sprites[1].expand_x);
                let m2xe = bit::bit_set(2, self.sprites[2].expand_x);
                let m3xe = bit::bit_set(3, self.sprites[3].expand_x);
                let m4xe = bit::bit_set(4, self.sprites[4].expand_x);
                let m5xe = bit::bit_set(5, self.sprites[5].expand_x);
                let m6xe = bit::bit_set(6, self.sprites[6].expand_x);
                let m7xe = bit::bit_set(7, self.sprites[7].expand_x);
                m0xe | m1xe | m2xe | m3xe | m4xe | m5xe | m6xe | m7xe
            },
            Reg::MM => 0xff, // DEFERRED collision
            Reg::MD => 0xff, // DEFERRED collision
            Reg::EC => self.border_color | 0xf0,
            Reg::B0C => self.background_color[0] | 0xf0,
            Reg::B1C => self.background_color[1] | 0xf0,
            Reg::B2C => self.background_color[2] | 0xf0,
            Reg::B3C => self.background_color[3] | 0xf0,
            Reg::MM0 => self.sprite_multicolor[0] | 0xf0,
            Reg::MM1 => self.sprite_multicolor[1] | 0xf0,
            Reg::M0C => self.sprites[0].color | 0xf0,
            Reg::M1C => self.sprites[1].color | 0xf0,
            Reg::M2C => self.sprites[2].color | 0xf0,
            Reg::M3C => self.sprites[3].color | 0xf0,
            Reg::M4C => self.sprites[4].color | 0xf0,
            Reg::M5C => self.sprites[5].color | 0xf0,
            Reg::M6C => self.sprites[6].color | 0xf0,
            Reg::M7C => self.sprites[7].color | 0xf0,
            Reg::IGNORE => 0xff,
        }
    }

    pub fn write(&mut self, reg: u8, value: u8) {
        match Reg::from(reg) {
            Reg::M0X => self.sprites[0].x = self.sprites[0].x & 0xff00 | (value as u16),
            Reg::M0Y => self.sprites[0].y = value,
            Reg::M1X => self.sprites[1].x = self.sprites[1].x & 0xff00 | (value as u16),
            Reg::M1Y => self.sprites[1].y = value,
            Reg::M2X => self.sprites[2].x = self.sprites[2].x & 0xff00 | (value as u16),
            Reg::M2Y => self.sprites[2].y = value,
            Reg::M3X => self.sprites[3].x = self.sprites[3].x & 0xff00 | (value as u16),
            Reg::M3Y => self.sprites[3].y = value,
            Reg::M4X => self.sprites[4].x = self.sprites[4].x & 0xff00 | (value as u16),
            Reg::M4Y => self.sprites[4].y = value,
            Reg::M5X => self.sprites[5].x = self.sprites[5].x & 0xff00 | (value as u16),
            Reg::M5Y => self.sprites[5].y = value,
            Reg::M6X => self.sprites[6].x = self.sprites[6].x & 0xff00 | (value as u16),
            Reg::M6Y => self.sprites[6].y = value,
            Reg::M7X => self.sprites[7].x = self.sprites[7].x & 0xff00 | (value as u16),
            Reg::M7Y => self.sprites[7].y = value,
            Reg::MX8 => {
                self.sprites[0].x = bit::bit_update16(self.sprites[0].x, 8, bit::bit_test(value, 0));
                self.sprites[1].x = bit::bit_update16(self.sprites[1].x, 8, bit::bit_test(value, 1));
                self.sprites[2].x = bit::bit_update16(self.sprites[2].x, 8, bit::bit_test(value, 2));
                self.sprites[3].x = bit::bit_update16(self.sprites[3].x, 8, bit::bit_test(value, 3));
                self.sprites[4].x = bit::bit_update16(self.sprites[4].x, 8, bit::bit_test(value, 4));
                self.sprites[5].x = bit::bit_update16(self.sprites[5].x, 8, bit::bit_test(value, 5));
                self.sprites[6].x = bit::bit_update16(self.sprites[6].x, 8, bit::bit_test(value, 6));
                self.sprites[7].x = bit::bit_update16(self.sprites[7].x, 8, bit::bit_test(value, 7));
            },
            Reg::CR1 => {
                self.raster_compare = bit::bit_update16(self.raster_compare, 8, bit::bit_test(value, 7));
                let mode = bit::bit_update(self.mode.value(), 2, bit::bit_test(value, 6));
                let mode2 = bit::bit_update(mode, 1, bit::bit_test(value, 5));
                self.mode = Mode::from(mode);
                self.enabled = bit::bit_test(value, 4);
                self.rsel = bit::bit_test(value, 3);
                let rsel = bit::bit_set(3, self.rsel);
                self.scroll_y = value & 0x07;
            }
            Reg::RASTER => self.raster_compare = self.raster_compare & 0xff00 | (value as u16),
            Reg::LPX => self.light_pen_pos[0] = value,
            Reg::LPY => self.light_pen_pos[1] = value,
            Reg::ME => {
                self.sprites[0].enabled = bit::bit_test(value, 0);
                self.sprites[1].enabled = bit::bit_test(value, 1);
                self.sprites[2].enabled = bit::bit_test(value, 2);
                self.sprites[3].enabled = bit::bit_test(value, 3);
                self.sprites[4].enabled = bit::bit_test(value, 4);
                self.sprites[5].enabled = bit::bit_test(value, 5);
                self.sprites[6].enabled = bit::bit_test(value, 6);
                self.sprites[7].enabled = bit::bit_test(value, 7);
            },
            Reg::CR2 => {
                let mode = bit::bit_update(self.mode.value(), 0, bit::bit_test(value, 4));
                self.mode = Mode::from(mode);
                self.csel = bit::bit_test(value, 3);
                self.scroll_x = value & 0x07;
            }
            Reg::MYE => {
                self.sprites[0].expand_y = bit::bit_test(value, 0);
                self.sprites[1].expand_y = bit::bit_test(value, 1);
                self.sprites[2].expand_y = bit::bit_test(value, 2);
                self.sprites[3].expand_y = bit::bit_test(value, 3);
                self.sprites[4].expand_y = bit::bit_test(value, 4);
                self.sprites[5].expand_y = bit::bit_test(value, 5);
                self.sprites[6].expand_y = bit::bit_test(value, 6);
                self.sprites[7].expand_y = bit::bit_test(value, 7);
            },
            Reg::MEMPTR => {
                self.video_matrix = (((value & 0xf0) >> 4) as u16) << 10;
                self.char_base = (((value & 0x0f) >> 1) as u16) << 11;
            },
            Reg::IRR => self.irq_status = value,
            Reg::IMR => self.irq_enable = value,
            Reg::MDP => {
                self.sprites[0].priority = bit::bit_test(value, 0);
                self.sprites[1].priority = bit::bit_test(value, 1);
                self.sprites[2].priority = bit::bit_test(value, 2);
                self.sprites[3].priority = bit::bit_test(value, 3);
                self.sprites[4].priority = bit::bit_test(value, 4);
                self.sprites[5].priority = bit::bit_test(value, 5);
                self.sprites[6].priority = bit::bit_test(value, 6);
                self.sprites[7].priority = bit::bit_test(value, 7);
            },
            Reg::MMC => {
                self.sprites[0].multicolor = bit::bit_test(value, 0);
                self.sprites[1].multicolor = bit::bit_test(value, 1);
                self.sprites[2].multicolor = bit::bit_test(value, 2);
                self.sprites[3].multicolor = bit::bit_test(value, 3);
                self.sprites[4].multicolor = bit::bit_test(value, 4);
                self.sprites[5].multicolor = bit::bit_test(value, 5);
                self.sprites[6].multicolor = bit::bit_test(value, 6);
                self.sprites[7].multicolor = bit::bit_test(value, 7);
            },
            Reg::MXE => {
                self.sprites[0].expand_x = bit::bit_test(value, 0);
                self.sprites[1].expand_x = bit::bit_test(value, 1);
                self.sprites[2].expand_x = bit::bit_test(value, 2);
                self.sprites[3].expand_x = bit::bit_test(value, 3);
                self.sprites[4].expand_x = bit::bit_test(value, 4);
                self.sprites[5].expand_x = bit::bit_test(value, 5);
                self.sprites[6].expand_x = bit::bit_test(value, 6);
                self.sprites[7].expand_x = bit::bit_test(value, 7);
            },
            Reg::MM => {},
            Reg::MD => {},
            Reg::EC => self.border_color = value & 0x0f,
            Reg::B0C => self.background_color[0] = value & 0x0f,
            Reg::B1C => self.background_color[1] = value & 0x0f,
            Reg::B2C => self.background_color[2] = value & 0x0f,
            Reg::B3C => self.background_color[3] = value & 0x0f,
            Reg::MM0 => self.sprite_multicolor[0] = value & 0x0f,
            Reg::MM1 => self.sprite_multicolor[1] = value & 0x0f,
            Reg::M0C => self.sprites[0].color = value & 0x0f,
            Reg::M1C => self.sprites[1].color = value & 0x0f,
            Reg::M2C => self.sprites[2].color = value & 0x0f,
            Reg::M3C => self.sprites[3].color = value & 0x0f,
            Reg::M4C => self.sprites[4].color = value & 0x0f,
            Reg::M5C => self.sprites[5].color = value & 0x0f,
            Reg::M6C => self.sprites[6].color = value & 0x0f,
            Reg::M7C => self.sprites[7].color = value & 0x0f,
            Reg::IGNORE => {},
        }
    }
}

