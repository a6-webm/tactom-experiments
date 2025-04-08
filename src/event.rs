use std::{io::Write, iter, thread::sleep, time::Duration};

use serialport::TTYPort;

const RAW_ENTER: u8 = 0xC0;
const RAW_EXIT: u8 = 0xF5;

/*
Placement of motors on the palm, with the plam facing the table:
Numbers represent the index of the motor
  Fingers
  0  1  2  3
  4  5  6  7
  8  9 10 11
  Wrist
*/

#[derive(Clone, Copy)]
pub enum EvType {
    _Go0 = 0, // Play motor with index "GO<index>"
    _Go1,
    _Go2,
    _Go3,
    _Go4,
    _Go5,
    _Go6,
    _Go7,
    _Go8,
    _Go9,
    _Go10,
    _Go11,
    EndGlyph, // Denote the end of a glyph
}

#[derive(Clone, Copy, Debug)]
pub struct Ev {
    pub ms_time: u16,
    pub ev_type: u8,
}

impl Ev {
    pub fn new(ms_time: u16, ev_type: u8) -> Self {
        Self { ms_time, ev_type }
    }
}

pub fn queue_events_as_raw(events: &[Ev], tty: &mut TTYPort) -> anyhow::Result<()> {
    let bytes_iter = events
        .iter()
        .flat_map(|ev| iter::once(ev.ev_type).chain(ev.ms_time.to_be_bytes()));
    let bytes: Vec<u8> = iter::once(RAW_ENTER)
        .chain(bytes_iter)
        .chain(iter::once(RAW_EXIT))
        .collect();
    for byte in bytes {
        tty.write_all(&[byte])?;
        tty.flush()?;
        sleep(Duration::from_millis(1)); // TODO fix this
    }
    Ok(())
}
