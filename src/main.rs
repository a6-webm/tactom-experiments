use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use csv::Writer;
use event::queue_events_as_raw;
use glyphs::{init_alphabets, println_glyph, Alphabet};
use rand::{random, rng, seq::SliceRandom};
use serde::Serialize;
use serialport::TTYPort;

mod event;
mod glyphs;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Exp {
    Calibrate,
    Distinguish,
    Alphabet,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// serial device to interface with tactom device
    #[arg(value_name = "TTY_DEV")]
    tty_path: PathBuf,
    /// Which experiment to run
    #[arg(value_enum, value_name = "EXPERIMENT")]
    exp: Exp,
    /// .csv file to record data to
    #[arg(value_name = "OUTPUT_FILE")]
    out_path: PathBuf,
}

#[derive(Serialize)]
struct DisinguishData {
    id: usize,
    glyph1: String,
    glyph2: String,
    glyph1_shown: bool,
    duration_ms: u128,
    correct: bool,
    unsure: bool,
}

#[derive(Serialize)]
struct AlphabetData {
    c: char,
    answer: char,
    duration_ms: u128,
    correct: bool,
    unsure: bool,
}

fn clear_term() {
    print!("\x1b[1;1H\x1b[2J");
}

fn flush() {
    io::stdout().flush().unwrap_or(())
}

fn distinguish_problem(
    tty: &mut TTYPort,
    a_bet: &Alphabet,
    prob: (&str, &str),
    q: usize,
    q_len: usize,
    prob_id: usize,
) -> anyhow::Result<DisinguishData> {
    let swap_glyphs: bool = random();
    let show_glyph_1: bool = random();
    let (glyph1, glyph2) = if swap_glyphs {
        (prob.1, prob.0)
    } else {
        (prob.0, prob.1)
    };
    println!("----- Question: {}/{} -----", q + 1, q_len);
    flush();
    sleep(Duration::from_secs_f32(1.0));
    println!("Glyph 1...");
    flush();
    queue_events_as_raw(a_bet.get_other_glyph(glyph1), tty)?;
    sleep(Duration::from_secs_f32(2.0));
    println!("Glyph 2...");
    flush();
    queue_events_as_raw(a_bet.get_other_glyph(glyph2), tty)?;
    sleep(Duration::from_secs_f32(2.0));
    if show_glyph_1 {
        println_glyph(a_bet.get_other_glyph(prob.0));
    } else {
        println_glyph(a_bet.get_other_glyph(prob.1));
    }
    let start = Instant::now();
    let mut answer = String::new();
    while answer != "1\n" && answer != "2\n" && answer != "?\n" {
        print!("Is the first or the second glyph played pictured above?\n(type '1','2' or '?' if you're unsure, then [Enter]): ");
        flush();
        answer = String::new();
        io::stdin().read_line(&mut answer)?;
    }
    let duration = Instant::now().duration_since(start);
    let unsure = answer == "?\n";
    let correct = ((answer == "1\n") ^ swap_glyphs ^ !show_glyph_1) && !unsure;
    Ok(DisinguishData {
        id: prob_id,
        glyph1: prob.0.to_owned(),
        glyph2: prob.1.to_owned(),
        glyph1_shown: show_glyph_1,
        duration_ms: duration.as_millis(),
        correct,
        unsure,
    })
}

fn distinguish_exp(
    mut out_writer: Writer<File>,
    mut tty: TTYPort,
    a_bet: &Alphabet,
) -> anyhow::Result<()> {
    clear_term();
    println!(
        "In this experiment, for each question, the Tactom device will play two \
glyphs (glyph 1 then glyph 2), with a short pause in-between.
You will then be shown a picture of one of the glyphs, and will need to answer \
if glyph 1 or 2 is pictured.

Example glyphs:"
    );
    println!("Clockwise swipe:");
    println_glyph(a_bet.get_other_glyph("clockwise"));
    println!("\nLeftwards swipe on the 2nd row");
    println_glyph(a_bet.get_other_glyph("row1_left"));
    println!();
    print!("Press [Enter] when you're ready to begin:");
    flush();
    io::stdin().read_line(&mut String::new())?;
    let mut problems: Vec<(usize, (&str, &str))> = [
        ("0", "11"),
        ("7", "4"),
        ("3", "2"),
        ("10", "11"),
        ("row0_right", "row0_left"),
        ("col2_up", "col2_down"),
        ("row2_right", "row2_left"),
        ("anticlockwise", "clockwise"),
        ("slash", "col3_down"),
        ("zag", "zig"),
        ("zag", "N"),
        ("anticlockwise", "N"),
    ]
    .into_iter()
    .enumerate()
    .collect();
    problems.shuffle(&mut rng());
    let q_len = problems.len();
    for (q, (p_id, prob)) in problems.into_iter().enumerate() {
        loop {
            clear_term();
            match distinguish_problem(&mut tty, a_bet, prob, q, q_len, p_id) {
                Ok(data) => {
                    // ----- WARN dbg
                    println!(
                        "{}",
                        if data.correct {
                            "dbg: Correct!"
                        } else {
                            "dbg: Wrong"
                        }
                    );
                    sleep(Duration::from_millis(500));
                    // -----
                    out_writer.serialize(data)?;
                    out_writer.flush()?;
                    break;
                }
                Err(e) => {
                    println!("An error has occured on problem {}, {}", p_id, e);
                    let mut answer = String::new();
                    while answer.to_lowercase() != "y\n" && answer.to_lowercase() != "n\n" {
                        print!("Would you like to retry this problem (otherwise, skip it)?[Y/n]: ");
                        flush();
                        answer = String::new();
                        io::stdin().read_line(&mut answer)?;
                    }
                    if answer.to_lowercase() == "n\n" {
                        out_writer.serialize(DisinguishData {
                            id: p_id,
                            glyph1: "error".to_owned(),
                            glyph2: "".to_owned(),
                            glyph1_shown: true,
                            duration_ms: 0,
                            correct: false,
                            unsure: false,
                        })?;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

fn alphabet_problem(
    tty: &mut TTYPort,
    a_bet: &Alphabet,
    prob: char,
    q: usize,
    q_len: usize,
) -> anyhow::Result<AlphabetData> {
    println!("----- Question: {}/{} -----", q + 1, q_len);
    flush();
    sleep(Duration::from_secs_f32(1.0));
    println!("Playing glyph...");
    flush();
    queue_events_as_raw(a_bet.get_glyph(prob), tty)?;
    sleep(Duration::from_secs_f32(2.0));
    let start = Instant::now();
    let mut answer = String::new();
    while answer
        .chars()
        .nth(0)
        .map(|c| c < 'a' || c > 'z')
        .unwrap_or(true)
        && answer.len() != 2
    {
        print!("What letter just played?\n(type 'a', 'b', 'c', ..., 'z' or '?' if you're unsure, then [Enter]): ");
        flush();
        answer = String::new();
        io::stdin().read_line(&mut answer)?;
    }
    let duration = Instant::now().duration_since(start);
    let answer = answer.chars().nth(0).unwrap();
    let unsure = answer == '?';
    let correct = (answer == prob) && !unsure;
    Ok(AlphabetData {
        c: prob,
        answer,
        duration_ms: duration.as_millis(),
        correct,
        unsure,
    })
}

fn alphabet_exp(
    mut out_writer: Writer<File>,
    mut tty: TTYPort,
    a_bet: &Alphabet,
) -> anyhow::Result<()> {
    clear_term();
    println!(
        "In this experiment, a glyph will be played on the Tactom device and you will have to \
identify what letter of the alphabet has been played.
You will be shown what all of the glyphs are before you begin answering questions.

Press [Enter] when you're ready to begin:"
    );
    flush();
    io::stdin().read_line(&mut String::new())?;

    for c in 'a'..='z' {
        clear_term();
        println!("----- Glyph '{}' -----", c);
        println_glyph(a_bet.get_glyph(c));
        flush();
        sleep(Duration::from_secs_f32(1.0));
        let mut answer = String::new();
        while answer.to_lowercase() != "n\n" {
            println!("Playing...");
            flush();
            sleep(Duration::from_secs_f32(2.0));
            print!("Would you like to replay this glyph (otherwise, advance to the next letter)?[Y/n]: ");
            flush();
            answer = String::new();
            io::stdin().read_line(&mut answer)?;
        }
    }

    let mut problems: Vec<char> = ('a'..='z').chain('a'..='z').collect();
    problems.shuffle(&mut rng());
    let q_len = problems.len();
    for (q, prob) in problems.into_iter().enumerate() {
        loop {
            clear_term();
            match alphabet_problem(&mut tty, a_bet, prob, q, q_len) {
                Ok(data) => {
                    out_writer.serialize(data)?;
                    break;
                }
                Err(e) => {
                    println!("An error has occured on problem {}, {}", prob as usize, e);
                    let mut answer = String::new();
                    while answer != "1\n" && answer != "2\n" {
                        print!("Would you like to retry this problem (otherwise, skip it)?[Y/n]: ");
                        flush();
                        answer = String::new();
                        io::stdin().read_line(&mut answer)?;
                    }
                    if answer.to_lowercase() == "n\n" {
                        out_writer.serialize(AlphabetData {
                            c: '%',
                            answer: ' ',
                            duration_ms: 0,
                            correct: false,
                            unsure: false,
                        })?;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if Path::exists(&cli.out_path) && cli.out_path != PathBuf::from("/dev/null") {
        return Err(anyhow!("OUTPUT_FILE path already exists"));
    }

    let tty = TTYPort::open(&serialport::new(cli.tty_path.to_string_lossy(), 115200))?;
    let out_writer = csv::WriterBuilder::new().from_path(cli.out_path)?;

    let alphabets = init_alphabets();

    match cli.exp {
        Exp::Calibrate => calibrate(tty, alphabets.get("distinguish").unwrap()),
        Exp::Distinguish => distinguish_exp(out_writer, tty, alphabets.get("distinguish").unwrap()),
        Exp::Alphabet => alphabet_exp(out_writer, tty, alphabets.get("alphabet_v1").unwrap()),
    }?;

    Ok(())
}

fn calibrate(mut tty: TTYPort, a_bet: &Alphabet) -> anyhow::Result<()> {
    let mut i: u8 = 0;
    loop {
        let glyph = a_bet.get_other_glyph(&i.to_string());
        queue_events_as_raw(glyph, &mut tty)?;
        clear_term();
        println_glyph(glyph);
        sleep(Duration::from_millis(500));
        i += 1;
        if i == 12 {
            i = 0;
        }
    }
}
