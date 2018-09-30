//! Handles the Monotron serial interface.
//!
//! This interface is both for fake keyboard input from your PC, and so that the Montron can
//! open, read and write files on your PC.
//!
//! Monotron is the master, and the PC is the slave. Monotron sends Requests,
//! and the PC sends Confirmations and Indications. Every Request has exactly
//! one Confirmation. The PC may send Indications asynchronously. Only one
//! Request may be in flight at any one time.
//!
//! Requests:
//! * OpenFile(filename: String, mode: Mode)
//! * CloseFile(handle: Handle)
//! * Read(handle: Handle, offset: usize)
//! * Checksum(handle: Handle)
//! * OpenDir()
//! * CloseDir(handle: Handle)
//! * ReadDir(handle: Handle)
//! Confirmations:
//! * Open(handle: Handle. error: Error)
//! * Close(error: Error)
//! * Read(data: String, error: Error)
//! * OpenDir(handle: Handle, error: Error)
//! * CloseDir(error: Error)
//! * ReadDir(filename: String, size: u32, mtime: Timestamp, type: Type)
//! Indications:
//! * Keypress(utf8_byte: u8)
#![no_std]

extern crate crc;

#[derive(Debug, Copy, Clone)]
pub enum Error {
    BadChecksum,
    BadHeader,
    BufferOverflow,
    FileNotFound,
    BadOffset,
}

#[derive(Debug)]
pub struct CommandWriter {
    bytes: [u8; 32],
    sent: usize,
    count: usize,
    had_escape: bool,
    crc: u16,
}

const PING_REQ: u8 = 0x01;
const PING_CFM: u8 = 0x81;
const END: u8 = 0xC0;
const ESC: u8 = 0xDB;
const ESC_END: u8 = 0xDC;
const ESC_ESC: u8 = 0xDD;

impl CommandWriter {
    pub fn new() -> CommandWriter {
        CommandWriter {
            bytes: [0u8; 32],
            sent: 0,
            count: 0,
            had_escape: false,
            crc: 0,
        }
    }

    pub fn reset(&mut self) {
        self.sent = 0;
        self.count = 0;
    }

    pub fn prep_for_send(&mut self) {
        self.sent = 0;
        // See https://crccalc.com/, marked CRC-16/X25
        self.crc = crc::crc16::checksum_x25(&self.bytes[0..self.count]);
    }

    pub fn send_ping_req(&mut self) {
        self.bytes[0] = PING_REQ;
        self.count = 1;
        self.prep_for_send();
    }

    pub fn send_ping_cfm(&mut self) {
        self.bytes[0] = PING_CFM;
        self.count = 1;
        self.prep_for_send();
    }

    fn escape_and_send(&mut self, to_send: u8) -> u8 {
        if !need_escape(to_send) {
            self.sent += 1;
            to_send
        } else {
            if !self.had_escape {
                self.had_escape = true;
                ESC
            } else {
                self.sent += 1;
                self.had_escape = false;
                escape(to_send)
            }
        }
    }

    pub fn get_byte(&mut self) -> Option<u8> {
        if self.sent == 0 {
            self.sent += 1;
            Some(END)
        } else if self.sent <= self.count {
            let to_send = self.bytes[self.sent - 1];
            Some(self.escape_and_send(to_send))
        } else if self.sent == (self.count + 1) {
            let to_send = (self.crc >> 8) as u8;
            Some(self.escape_and_send(to_send))
        } else if self.sent == (self.count + 2) {
            let to_send = (self.crc >> 0) as u8;
            Some(self.escape_and_send(to_send))
        } else if self.sent == (self.count + 3) {
            self.sent += 1;
            Some(END)
        } else {
            None
        }
    }
}

fn need_escape(byte: u8) -> bool {
    match byte {
        END => true,
        ESC => true,
        _ => false,
    }
}

fn escape(byte: u8) -> u8 {
    match byte {
        END => ESC_END,
        ESC => ESC_ESC,
        x => x,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_ping_req() {
        let mut cw = CommandWriter::new();
        cw.send_ping_req();
        assert_eq!(cw.get_byte(), Some(END));
        assert_eq!(cw.get_byte(), Some(PING_REQ));
        assert_eq!(cw.get_byte(), Some(0xE1));
        assert_eq!(cw.get_byte(), Some(0xF1));
        assert_eq!(cw.get_byte(), Some(END));
        assert_eq!(cw.get_byte(), None);
    }

    #[test]
    fn basic_ping_cfm() {
        let mut cw = CommandWriter::new();
        cw.send_ping_cfm();
        assert_eq!(cw.get_byte(), Some(END));
        assert_eq!(cw.get_byte(), Some(PING_CFM));
        assert_eq!(cw.get_byte(), Some(0x65));
        assert_eq!(cw.get_byte(), Some(0xF9));
        assert_eq!(cw.get_byte(), Some(END));
        assert_eq!(cw.get_byte(), None);
    }
}
