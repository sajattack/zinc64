/*
 * Copyright (c) 2016-2018 Sebastian Jastrzebski. All rights reserved.
 *
 * This file is part of zinc64.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use std::cell::RefCell;
use std::rc::Rc;

use bit_field::BitField;
use core::{IoPort, Pin};

use super::Tape;

// DEFERRED device: datassette test cases

/*
  +---------+---+------------+--------------------------------------------+
  |  NAME   |BIT| DIRECTION  |                 DESCRIPTION                |
  +---------+---+------------+--------------------------------------------+
  |  LORAM  | 0 |   OUTPUT   | Control for RAM/ROM at $A000-$BFFF         |
  |  HIRAM  | 1 |   OUTPUT   | Control for RAM/ROM at $E000-$FFFF         |
  |  CHAREN | 2 |   OUTPUT   | Control for I/O/ROM at $D000-$DFFF         |
  |         | 3 |   OUTPUT   | Cassette write line                        |
  |         | 4 |   INPUT    | Cassette switch sense (0=play button down) |
  |         | 5 |   OUTPUT   | Cassette motor control (0=motor spins)     |
  +---------+---+------------+--------------------------------------------+
*/

const DUTY_CYCLE: u32 = 50;

#[derive(Copy, Clone)]
enum ControlPort {
    CassetteSwitch = 4,
    CassetteMotor = 5,
}

impl ControlPort {
    pub fn value(&self) -> usize {
        *self as usize
    }
}

pub struct Pulse {
    low_cycles: u32,
    remaining_cycles: u32,
}

impl Pulse {
    pub fn new(length: u32, duty: u32) -> Pulse {
        Pulse {
            low_cycles: length * (100 - duty) / 100,
            remaining_cycles: length,
        }
    }

    #[inline]
    pub fn is_done(&self) -> bool {
        self.remaining_cycles == 0
    }

    #[inline]
    pub fn advance(&mut self) -> bool {
        self.remaining_cycles -= 1;
        if self.low_cycles == 0 {
            true
        } else {
            self.low_cycles -= 1;
            false
        }
    }
}

pub struct Datassette {
    // Dependencies
    cia_flag: Rc<RefCell<Pin>>,
    cpu_io_port: Rc<RefCell<IoPort>>,
    // Runtime State
    playing: bool,
    tape: Option<Box<Tape>>,
    current_pulse: Pulse,
}

impl Datassette {
    pub fn new(cia_flag: Rc<RefCell<Pin>>, cpu_io_port: Rc<RefCell<IoPort>>) -> Datassette {
        Datassette {
            cia_flag,
            cpu_io_port,
            playing: false,
            tape: None,
            current_pulse: Pulse::new(0, DUTY_CYCLE),
        }
    }

    pub fn attach(&mut self, tape: Box<Tape>) {
        self.tape = Some(tape);
    }

    pub fn clock(&mut self) {
        if self.is_playing() && self.tape.is_some() {
            if self.current_pulse.is_done() {
                let pulse_maybe = if let Some(ref mut tape) = self.tape {
                    tape.read_pulse()
                } else {
                    None
                };
                if let Some(pulse) = pulse_maybe {
                    self.current_pulse = Pulse::new(pulse, DUTY_CYCLE);
                } else {
                    self.stop();
                }
            }
            if !self.current_pulse.is_done() {
                self.cia_flag
                    .borrow_mut()
                    .set_active(self.current_pulse.advance());
            }
        }
    }

    pub fn detach(&mut self) {
        self.stop();
        self.tape = None;
    }

    pub fn is_playing(&self) -> bool {
        // Cassette motor control (0=motor spins)
        let motor_on = !self.cpu_io_port
            .borrow()
            .get_value()
            .get_bit(ControlPort::CassetteMotor.value());
        self.playing & motor_on
    }

    pub fn play(&mut self) {
        info!(target: "device", "Starting datassette");
        if self.tape.is_some() {
            self.cpu_io_port
                .borrow_mut()
                .set_input_bit(ControlPort::CassetteSwitch.value(), false);
            self.playing = true;
        }
    }

    pub fn reset(&mut self) {
        self.cpu_io_port
            .borrow_mut()
            .set_input_bit(ControlPort::CassetteSwitch.value(), true);
        self.playing = false;
        self.current_pulse = Pulse::new(0, DUTY_CYCLE);
        if let Some(ref mut tape) = self.tape {
            tape.seek(0);
        }
    }

    pub fn stop(&mut self) {
        info!(target: "device", "Stopping datassette");
        self.cpu_io_port
            .borrow_mut()
            .set_input_bit(ControlPort::CassetteSwitch.value(), true);
        self.playing = false;
    }
}
