// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A serial port as a [`Line`], through `serialport` built without its
//! libudev feature: nothing here links C, and a port is opened by the name
//! it is configured with rather than found by enumeration.

use super::{Line, Mstp};
use crate::master::Config;
use serialport::SerialPort;
use std::io::{self, ErrorKind, Read, Write};
use std::time::Duration;

impl Line for Box<dyn SerialPort> {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        self.set_timeout(timeout)?;
        match Read::read(self, buf) {
            Err(e) if e.kind() == ErrorKind::TimedOut => Ok(0),
            other => other,
        }
    }

    fn write(&mut self, octets: &[u8]) -> io::Result<()> {
        self.write_all(octets)?;
        self.flush()
    }
}

impl Mstp<Box<dyn SerialPort>> {
    /// Opens `path` (`/dev/ttyUSB0`, `COM3`) at `baud` — 9600, 19200, 38400,
    /// 57600, 76800 or 115200 (§9.2.1) — eight data bits, no parity, one
    /// stop bit, as MS/TP requires.
    ///
    /// # Errors
    ///
    /// When the port cannot be opened.
    pub fn open(path: &str, baud: u32, config: Config) -> io::Result<Self> {
        let port = serialport::new(path, baud)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(Duration::from_millis(10))
            .open()?;
        Ok(Self::new(port, config))
    }
}
