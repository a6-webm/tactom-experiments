use std::{fs::File, io::Write, iter};

const RAW_ENTER: u8 = 0xC0;
const RAW_EXIT: u8 = 0xC1;

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
    Go0 = 0, // Play motor with index "GO<index>"
    Go1,
    Go2,
    Go3,
    Go4,
    Go5,
    Go6,
    Go7,
    Go8,
    Go9,
    Go10,
    Go11,
    EndGlyph, // Denote the end of a glyph
}

#[derive(Clone, Copy)]
pub struct Ev {
    pub ms_time: u16,
    pub ev_type: u8,
}

impl Ev {
    pub fn new(ms_time: u16, ev_type: u8) -> Self {
        Self { ms_time, ev_type }
    }
}

pub fn queue_events_as_raw(events: &[Ev], tty: &mut File) -> anyhow::Result<()> {
    let bytes_iter = events
        .iter()
        .flat_map(|ev| iter::once(ev.ev_type).chain(ev.ms_time.to_be_bytes()));
    let bytes: Vec<u8> = iter::once(RAW_ENTER)
        .chain(bytes_iter)
        .chain(iter::once(RAW_EXIT))
        .collect();
    tty.write_all(&bytes)?;
    Ok(())
}
