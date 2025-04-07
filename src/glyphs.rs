use std::{collections::HashMap, iter};

use colored::Colorize;
use tabled::settings::{width, Style};

use crate::event::{Ev, EvType::EndGlyph};

pub struct Alphabet {
    ascii_block: Vec<Vec<Ev>>, // from ' ' to '~' inclusive
    char_map: HashMap<char, Vec<Ev>>,
    pub other_map: HashMap<String, Vec<Ev>>,
    unknown_glyph: Vec<Ev>,
}

impl Default for Alphabet {
    fn default() -> Self {
        let default_glyph = vec![Ev::new(0, 0), Ev::new(200, EndGlyph as u8)];
        let mut char_map = HashMap::new();
        char_map.insert('\n', default_glyph.clone());
        Self {
            ascii_block: vec![default_glyph.clone(); 95],
            char_map,
            other_map: HashMap::new(),
            unknown_glyph: default_glyph.clone(),
        }
    }
}

impl Alphabet {
    fn add_other_glyphs(&mut self, gls: Vec<(&str, Vec<Ev>)>) {
        for (s, evs) in gls.into_iter() {
            self.add_other_glyph(s, evs);
        }
    }

    fn add_other_glyph(&mut self, s: &str, g: Vec<Ev>) {
        self.other_map.insert(s.to_owned(), g);
    }

    pub fn get_glyph(&self, c: char) -> &[Ev] {
        if c >= ' ' && c <= '~' {
            let idx = c as usize - ' ' as usize;
            &self.ascii_block[idx]
        } else {
            self.char_map.get(&c).unwrap_or(&self.unknown_glyph)
        }
    }

    pub fn get_other_glyph(&self, s: &str) -> &[Ev] {
        self.other_map
            .get(&(s.to_owned()))
            .unwrap_or(&self.unknown_glyph)
    }
}

/// linear rgb
fn ch_to_lin(ch: u8) -> f32 {
    let s = ch as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// linear rgb
fn lin_to_ch(lin: f32) -> u8 {
    let s = if lin <= 0.0031308 {
        lin * 12.92
    } else {
        1.055 * lin.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0) as u8
}

fn color_interpolate(c1: (u8, u8, u8), c2: (u8, u8, u8), f: f32) -> (u8, u8, u8) {
    let interpolate = |ch1: u8, ch2: u8, f: f32| {
        let lin1 = ch_to_lin(ch1);
        let lin2 = ch_to_lin(ch2);
        return lin_to_ch((lin2 - lin1) * f + lin1);
    };
    (
        interpolate(c1.0, c2.0, f),
        interpolate(c1.1, c2.1, f),
        interpolate(c1.2, c2.2, f),
    )
}

pub fn println_glyph(glyph: &[Ev]) {
    let c1 = (0, 255, 0);
    let c2 = (0, 0, 255);
    let mut places: Vec<Vec<usize>> = vec![];
    for _ in 0..=(EndGlyph as usize) {
        places.push(vec![]);
    }
    let mut idx = 0;
    for ev in glyph {
        places[ev.ev_type as usize].push(idx);
        idx += 1;
    }

    let mut tb_builder = tabled::builder::Builder::default();

    let len = (idx - 1) as f32;

    const ROWS: usize = 3;
    const COLS: usize = 4;
    for y in 0..ROWS {
        let mut row = vec![];
        for x in 0..COLS {
            let mut uh = String::new();
            for occ in places[x + y * COLS].iter() {
                let c = color_interpolate(c1, c2, *occ as f32 / len);
                uh += &occ.to_string().truecolor(c.0, c.1, c.2).to_string();
                uh += "\n";
            }
            uh.pop();
            row.push(uh);
        }
        tb_builder.push_record(row);
    }

    let mut table = tb_builder.build();
    table
        .with(Style::modern_rounded())
        .with(width::Justify::max());
    println!("{}", table);
}

fn equal_spaced_evs(evs: &[u8], space_ms: u16) -> Vec<Ev> {
    evs.iter()
        .cloned()
        .chain(iter::once(EndGlyph as u8))
        .enumerate()
        .map(|(i, et)| Ev::new(i as u16 * space_ms, et))
        .collect()
}

pub fn retime_eq_spaced(glyph: &[Ev], space_ms: u16) -> Vec<Ev> {
    glyph
        .iter()
        .map(|ev| ev.ev_type)
        .enumerate()
        .map(|(i, et)| Ev::new(i as u16 * space_ms, et))
        .collect()
}

fn stitch_evs(glyphs: &[&[Ev]]) -> Vec<Ev> {
    let mut out = vec![];
    let mut time: u16 = 0;
    let mut end_time: u16 = 0;
    for g in glyphs {
        for mut ev in g.iter().cloned() {
            if ev.ev_type == EndGlyph as u8 {
                end_time = time + ev.ms_time;
                time += ev.ms_time;
            } else {
                ev.ms_time += time;
                out.push(ev);
            }
        }
    }
    out.push(Ev::new(end_time, EndGlyph as u8));
    out
}

pub fn glyph_duration(glyph: &[Ev]) -> u16 {
    glyph.last().map(|ev| ev.ms_time).unwrap_or(0)
}

pub fn init_alphabets() -> HashMap<String, Alphabet> {
    let mut map = HashMap::new();

    let mut distinguish = Alphabet::default();
    for i in 0..12 {
        distinguish
            .other_map
            .insert(i.to_string(), equal_spaced_evs(&[i], 30));
    }
    distinguish.add_other_glyphs(vec![
        ("col0_up", equal_spaced_evs(&[8, 4, 0], 30)),
        ("col1_up", equal_spaced_evs(&[9, 5, 1], 30)),
        ("col2_up", equal_spaced_evs(&[10, 6, 2], 30)),
        ("col3_up", equal_spaced_evs(&[11, 7, 3], 30)),
        ("col0_down", equal_spaced_evs(&[0, 4, 8], 30)),
        ("col1_down", equal_spaced_evs(&[1, 5, 9], 30)),
        ("col2_down", equal_spaced_evs(&[2, 6, 10], 30)),
        ("col3_down", equal_spaced_evs(&[3, 7, 11], 30)),
        ("row0_right", equal_spaced_evs(&[0, 1, 2, 3], 30)),
        ("row1_right", equal_spaced_evs(&[4, 5, 6, 7], 30)),
        ("row2_right", equal_spaced_evs(&[8, 9, 10, 11], 30)),
        ("row0_left", equal_spaced_evs(&[3, 2, 1, 0], 30)),
        ("row1_left", equal_spaced_evs(&[7, 6, 5, 4], 30)),
        ("row2_left", equal_spaced_evs(&[11, 10, 9, 8], 30)),
        (
            "clockwise",
            equal_spaced_evs(&[0, 1, 2, 3, 7, 11, 10, 9, 8, 4, 0], 30),
        ),
        (
            "anticlockwise",
            equal_spaced_evs(&[0, 4, 8, 9, 10, 11, 7, 3, 2, 1, 0], 30),
        ),
        ("slash", equal_spaced_evs(&[3, 6, 5, 8], 30)),
        ("rev_slash", equal_spaced_evs(&[8, 5, 6, 3], 30)),
        ("backslash", equal_spaced_evs(&[0, 5, 6, 11], 30)),
        ("rev_backslash", equal_spaced_evs(&[11, 6, 5, 0], 30)),
    ]);
    distinguish.add_other_glyphs(vec![
        (
            "N",
            stitch_evs(&[
                distinguish.get_other_glyph("col0_up"),
                distinguish.get_other_glyph("col1_down"),
                distinguish.get_other_glyph("col2_up"),
            ]),
        ),
        (
            "flipped_N",
            stitch_evs(&[
                distinguish.get_other_glyph("col2_up"),
                distinguish.get_other_glyph("col1_down"),
                distinguish.get_other_glyph("col0_up"),
            ]),
        ),
        (
            "zig",
            stitch_evs(&[
                distinguish.get_other_glyph("row0_right"),
                distinguish.get_other_glyph("row1_left"),
                distinguish.get_other_glyph("row2_right"),
            ]),
        ),
        (
            "zag",
            stitch_evs(&[
                distinguish.get_other_glyph("row0_left"),
                distinguish.get_other_glyph("row1_right"),
                distinguish.get_other_glyph("row2_left"),
            ]),
        ),
    ]);
    map.insert("distinguish".to_owned(), distinguish);

    let mut roud_graff = Alphabet::default(); // short for "Roudaut graffiti"
    for i in 0..12 {
        roud_graff
            .other_map
            .insert(i.to_string(), equal_spaced_evs(&[i], 100));
    }
    roud_graff.ascii_block = vec![
        roud_graff.unknown_glyph.to_owned(),      // <space>
        roud_graff.unknown_glyph.to_owned(),      // !
        roud_graff.unknown_glyph.to_owned(),      // "
        roud_graff.unknown_glyph.to_owned(),      // #
        roud_graff.unknown_glyph.to_owned(),      // $
        roud_graff.unknown_glyph.to_owned(),      // %
        roud_graff.unknown_glyph.to_owned(),      // &
        roud_graff.unknown_glyph.to_owned(),      // '
        roud_graff.unknown_glyph.to_owned(),      // (
        roud_graff.unknown_glyph.to_owned(),      // )
        roud_graff.unknown_glyph.to_owned(),      // *
        roud_graff.unknown_glyph.to_owned(),      // +
        roud_graff.unknown_glyph.to_owned(),      // ,
        roud_graff.unknown_glyph.to_owned(),      // -
        roud_graff.unknown_glyph.to_owned(),      // .
        roud_graff.unknown_glyph.to_owned(),      // /
        roud_graff.unknown_glyph.to_owned(),      // 0
        roud_graff.unknown_glyph.to_owned(),      // 1
        roud_graff.unknown_glyph.to_owned(),      // 2
        roud_graff.unknown_glyph.to_owned(),      // 3
        roud_graff.unknown_glyph.to_owned(),      // 4
        roud_graff.unknown_glyph.to_owned(),      // 5
        roud_graff.unknown_glyph.to_owned(),      // 6
        roud_graff.unknown_glyph.to_owned(),      // 7
        roud_graff.unknown_glyph.to_owned(),      // 8
        roud_graff.unknown_glyph.to_owned(),      // 9
        roud_graff.unknown_glyph.to_owned(),      // :
        roud_graff.unknown_glyph.to_owned(),      // ;
        roud_graff.unknown_glyph.to_owned(),      // <
        roud_graff.unknown_glyph.to_owned(),      // =
        roud_graff.unknown_glyph.to_owned(),      // >
        roud_graff.unknown_glyph.to_owned(),      // ?
        roud_graff.unknown_glyph.to_owned(),      // @
        roud_graff.unknown_glyph.to_owned(),      // A
        roud_graff.unknown_glyph.to_owned(),      // B
        roud_graff.unknown_glyph.to_owned(),      // C
        roud_graff.unknown_glyph.to_owned(),      // D
        roud_graff.unknown_glyph.to_owned(),      // E
        roud_graff.unknown_glyph.to_owned(),      // F
        roud_graff.unknown_glyph.to_owned(),      // G
        roud_graff.unknown_glyph.to_owned(),      // H
        roud_graff.unknown_glyph.to_owned(),      // I
        roud_graff.unknown_glyph.to_owned(),      // J
        roud_graff.unknown_glyph.to_owned(),      // K
        roud_graff.unknown_glyph.to_owned(),      // L
        roud_graff.unknown_glyph.to_owned(),      // M
        roud_graff.unknown_glyph.to_owned(),      // N
        roud_graff.unknown_glyph.to_owned(),      // O
        roud_graff.unknown_glyph.to_owned(),      // P
        roud_graff.unknown_glyph.to_owned(),      // Q
        roud_graff.unknown_glyph.to_owned(),      // R
        roud_graff.unknown_glyph.to_owned(),      // S
        roud_graff.unknown_glyph.to_owned(),      // T
        roud_graff.unknown_glyph.to_owned(),      // U
        roud_graff.unknown_glyph.to_owned(),      // V
        roud_graff.unknown_glyph.to_owned(),      // W
        roud_graff.unknown_glyph.to_owned(),      // X
        roud_graff.unknown_glyph.to_owned(),      // Y
        roud_graff.unknown_glyph.to_owned(),      // Z
        roud_graff.unknown_glyph.to_owned(),      // [
        roud_graff.unknown_glyph.to_owned(),      // \
        roud_graff.unknown_glyph.to_owned(),      // ]
        roud_graff.unknown_glyph.to_owned(),      // ^
        roud_graff.unknown_glyph.to_owned(),      // _
        roud_graff.unknown_glyph.to_owned(),      // `
        equal_spaced_evs(&[8, 4, 1, 6, 11], 150), // a
        equal_spaced_evs(&[0, 4, 8, 4, 5, 6, 7, 11, 10, 9, 8], 150), // b
        equal_spaced_evs(&[3, 2, 1, 0, 4, 8, 9, 10, 11], 150), // c
        equal_spaced_evs(&[8, 4, 0, 1, 2, 3, 7, 11, 10, 9, 8], 150), // d
        equal_spaced_evs(&[4, 5, 6, 7, 3, 2, 1, 0, 4, 8, 9, 10, 11], 150), // e
        equal_spaced_evs(&[3, 2, 1, 0, 4, 8], 150), // f
        equal_spaced_evs(&[1, 0, 4, 8, 9, 10, 11, 7, 6], 150), // g
        equal_spaced_evs(&[0, 4, 8, 4, 5, 6, 7, 11], 150), // h
        equal_spaced_evs(&[0, 4, 8], 150),        // i
        equal_spaced_evs(&[3, 7, 11, 10, 9, 8], 150), // j
        equal_spaced_evs(&[3, 6, 9, 8, 4, 0, 1, 6, 11], 150), // k
        equal_spaced_evs(&[0, 4, 8, 9, 10, 11], 150), // l
        equal_spaced_evs(&[8, 4, 0, 1, 5, 6, 2, 3, 7, 11], 150), // m
        equal_spaced_evs(&[8, 4, 0, 1, 5, 10, 11, 7, 3], 150), // n
        equal_spaced_evs(&[1, 0, 4, 8, 9, 10, 11, 7, 3, 2], 150), // o
        equal_spaced_evs(&[8, 4, 0, 1, 2, 3, 7, 6, 5, 4], 150), // p
        equal_spaced_evs(&[3, 2, 1, 0, 4, 5, 6, 7, 3, 7, 11], 150), // q
        equal_spaced_evs(&[8, 4, 0, 1, 2, 3, 7, 6, 5, 4, 5, 10], 150), // r
        equal_spaced_evs(&[3, 2, 1, 0, 4, 5, 6, 7, 11, 10, 9, 8], 150), // s
        equal_spaced_evs(&[0, 1, 2, 3, 7, 11], 150), // t
        equal_spaced_evs(&[0, 4, 8, 9, 10, 11, 7, 3], 150), // u
        equal_spaced_evs(&[0, 4, 9, 6, 3], 150),  // v
        equal_spaced_evs(&[0, 4, 8, 9, 5, 6, 10, 11, 7, 3], 150), // w
        equal_spaced_evs(&[0, 5, 10, 6, 2, 5, 8], 150), // x
        equal_spaced_evs(&[0, 4, 5, 6, 7, 3, 7, 11, 10, 9, 6, 3], 150), // y
        equal_spaced_evs(&[0, 1, 2, 3, 7, 6, 5, 4, 8, 9, 10, 11], 150), // z
        roud_graff.unknown_glyph.to_owned(),      // {
        roud_graff.unknown_glyph.to_owned(),      // |
        roud_graff.unknown_glyph.to_owned(),      // }
        roud_graff.unknown_glyph.to_owned(),      // ~
    ];
    map.insert("roud_graff".to_owned(), roud_graff);

    map
}
