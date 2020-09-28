use image;
use sysfs_gpio::{Direction, Pin, Result};
use spidev::{Spidev, SpidevOptions, SpiModeFlags};

use std::time;
use std::thread;
use std::io::prelude::*;

const DISPLAY_WIDTH: usize = 400;
const DISPLAY_HEIGHT: usize = 300;
const WAIT_DURATION: time::Duration = time::Duration::from_millis(200);

fn is_black(img: &image::GrayImage, x: u32, y: u32) -> bool {
    let image::Luma([p]) = img.get_pixel(x, y);
    if *p < 128 {
        true
    } else {
        false
    }
}

pub struct EPaper42Driver {
    busy_pin: Pin,
    rst_pin: Pin,
    dc_pin: Pin,
    spidev: Spidev
}

impl EPaper42Driver {
    pub fn new(busy_port: u64, rst_port: u64, dc_port: u64, spi_devname: &str) -> EPaper42Driver {
        let spidev = Spidev::open(spi_devname).unwrap();

        EPaper42Driver {
            busy_pin: Pin::new(busy_port),
            rst_pin: Pin::new(rst_port),
            dc_pin: Pin::new(dc_port),
            spidev
        }
    }

    pub fn init(&mut self) -> Result<()> {
        self.busy_pin.export()?;
        self.rst_pin.export()?;
        self.dc_pin.export()?;

        self.busy_pin.set_direction(Direction::In)?;
        self.rst_pin.set_direction(Direction::Out)?;
        self.dc_pin.set_direction(Direction::Out)?;

        self.spidev.configure(&SpidevOptions::new()
            .bits_per_word(0).max_speed_hz(4000000).mode(SpiModeFlags::SPI_MODE_0)
            .lsb_first(false)
            .build())?;

        Ok(())
    }

    pub fn is_busy(&self) -> Result<bool> {
        match self.busy_pin.get_value() {
            Ok(0) => Ok(true),
            Ok(1) => Ok(false),
            Ok(_) => Err(sysfs_gpio::Error::Unexpected("Unexpected value read".to_string())),
            Err(e) => Err(e)
        }
    }

    pub fn reset(&self) -> Result<()> {
        self.rst_pin.set_value(1)?;
        thread::sleep(WAIT_DURATION);
        self.rst_pin.set_value(0)?;
        thread::sleep(WAIT_DURATION);
        self.rst_pin.set_value(1)?;
        thread::sleep(WAIT_DURATION);

        Ok(())
    }

    pub fn send_command_byte(&mut self, byte: u8) -> Result<()> {
        self.dc_pin.set_value(0)?;
        self.spidev.write(&[byte])?;

        Ok(())
    }
    pub fn send_data_byte(&mut self, byte: u8) -> Result<()> {
        self.dc_pin.set_value(1)?;
        self.spidev.write(&[byte])?;

        Ok(())
    }

    pub fn send_data(&mut self, bytes: &[u8]) -> Result<()> {
        self.dc_pin.set_value(1)?;
        self.spidev.write(bytes)?;

        Ok(())
    }

    pub fn first_sequence(&mut self) -> Result<()> {
        self.send_command_byte(0x06)?;
        
        self.send_data_byte(0x17)?;
        self.send_data_byte(0x17)?;
        self.send_data_byte(0x17)?;

        self.send_command_byte(0x04)?;

        self.wait_busy()?;

        self.send_command_byte(0x00)?;
        self.send_data_byte(0x0f)?;

        Ok(())
    }

    pub fn wait_busy(&self) -> Result<()> {
        loop {
            let busy = self.is_busy()?;
            if !busy {
                return Ok(())
            }

            thread::sleep(WAIT_DURATION);
            println!(".");
        }
    }

    pub fn clear_display(&mut self) -> Result<()> {
        let empty_line = [0xffu8; DISPLAY_WIDTH / 8];

        self.send_command_byte(0x10)?;
        for _ in 0..DISPLAY_HEIGHT {
            self.send_data(&empty_line)?;
        }

        self.send_command_byte(0x13)?;
        for _ in 0..DISPLAY_HEIGHT {
            self.send_data(&empty_line)?;
        }

        self.send_command_byte(0x12)?;
        self.wait_busy()?;

        Ok(())
    }

    pub fn print_tricolor(&mut self) -> Result<()> {
        let empty_line = [0xffu8; DISPLAY_WIDTH / 8];
        let full_line = [0x00u8; DISPLAY_WIDTH / 8];

        self.send_command_byte(0x10)?;
        for y in 0..DISPLAY_HEIGHT {
            if y < DISPLAY_HEIGHT / 3 {
                self.send_data(&empty_line)?;
            } else {
                self.send_data(&full_line)?;
            }
        }

        self.send_command_byte(0x13)?;
        for y in 0..DISPLAY_HEIGHT {
            if y < DISPLAY_HEIGHT / 3 * 2 {
                self.send_data(&empty_line)?;
            } else {
                self.send_data(&full_line)?;
            }
        }

        self.send_command_byte(0x12)?;
        self.wait_busy()?;

        Ok(())
    }

    pub fn print_image(&mut self, img: &image::GrayImage) -> Result<()> {
        let empty_line = [0xffu8; DISPLAY_WIDTH / 8];

        if img.width() != DISPLAY_WIDTH as u32 || img.height() != DISPLAY_HEIGHT as u32 {
            return Err(sysfs_gpio::Error::Unsupported("Wrong image size".to_string()));
        }

        let mut buf = Vec::new();
        for y in 0..DISPLAY_HEIGHT {
            let mut xbuf = [0xffu8; DISPLAY_WIDTH / 8];
            for x in 0..(DISPLAY_WIDTH / 8) {
                let mut dat = 0xffu8;

                for b in 0..8 {
                    if !is_black(&img, (x * 8 + b) as u32, y as u32) {
                        dat &= !(1 << b);
                    }
                }
                xbuf[x] = dat;

                print!("{},", xbuf[x]);
            }
            println!("");
            buf.push(xbuf);
        }

        self.send_command_byte(0x10)?;
        for y in 0..DISPLAY_HEIGHT {
            self.send_data(&buf[y])?;
        }

        self.send_command_byte(0x13)?;
        for _ in 0..DISPLAY_HEIGHT {
            self.send_data(&empty_line)?;
        }

        self.send_command_byte(0x12)?;
        self.wait_busy()?;

        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        self.send_command_byte(0x02)?;
        self.wait_busy()?;
        self.send_command_byte(0x07)?;
        self.send_data_byte(0xa5)?;

        Ok(())
    }
}
