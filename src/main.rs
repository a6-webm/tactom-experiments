use std::{
    fs::File,
    io::{self, stdin, Write},
    iter,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use csv::Writer;
use event::{queue_events_as_raw, Ev};
use glyphs::{glyph_duration, init_alphabets, println_glyph, retime_eq_spaced, Alphabet};
use rand::{random, rng, seq::SliceRandom};
use serde::Serialize;
use serialport::TTYPort;
use tokio::{sync::RwLock, time::sleep};

mod event;
mod glyphs;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Exp {
    Dropout,
    Draw,
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
struct DropoutData {
    id: usize,
    glyph: String,
    drop_glyph: String,
    speed: u16,
    drop_played: bool,
    duration_ms: u128,
    correct: bool,
    unsure: bool,
}

#[derive(Serialize)]
struct DrawData {
    glyph: char,
    speed: u16,
    duration_ms: u128,
    pathiness: u8,
}

#[derive(Serialize, Debug)]
struct AlphabetData {
    c: char,
    speed: u16,
    answer: char,
    duration_ms: u128,
    occurrence: usize,
    correct: bool,
    unsure: bool,
}

fn clear_term() {
    print!("\x1b[1;1H\x1b[2J");
}

fn flush() {
    io::stdout().flush().unwrap_or(())
}

fn ask<S: AsRef<str>>(question: &str, possible_answers: &[S]) -> anyhow::Result<String> {
    let mut answer = String::new();
    loop {
        print!("{}", question);
        flush();
        stdin().read_line(&mut answer)?;
        let t_answer = answer.trim().to_lowercase();
        for a in possible_answers {
            if a.as_ref() == t_answer {
                return Ok(t_answer.to_owned());
            }
        }
        answer = String::new();
    }
}

async fn play_and_wait(tty: &mut TTYPort, glyph: &[Ev]) -> anyhow::Result<()> {
    queue_events_as_raw(glyph, tty)?;
    let g_dur = glyph_duration(glyph);
    if g_dur >= 1000 {
        sleep(Duration::from_secs_f32(1.0) + Duration::from_millis(g_dur as u64)).await;
    } else {
        sleep(Duration::from_secs_f32(2.0)).await;
    }
    Ok(())
}

async fn calibrate_until_enter(tty: &mut TTYPort, a_bet: &Alphabet) -> anyhow::Result<()> {
    let done = Arc::new(RwLock::new(false));
    let done_1 = done.clone();
    tokio::spawn(async move {
        stdin().read_line(&mut String::new()).unwrap_or(0);
        *done_1.write().await = true;
    });
    let mut i: u8 = 0;
    loop {
        let glyph = a_bet.get_other_glyph(&i.to_string());
        queue_events_as_raw(glyph, tty)?;
        sleep(Duration::from_millis(150)).await;
        if *done.read().await {
            break;
        }
        i += 1;
        if i == 12 {
            i = 0;
        }
    }
    Ok(())
}

async fn dropout_problem(
    tty: &mut TTYPort,
    a_bet: &Alphabet,
    prob: (&str, &str, u16),
    q: usize,
    q_len: usize,
    prob_id: usize,
) -> anyhow::Result<DropoutData> {
    let play_dropout: bool = random();
    let swap_glyphs: bool = random();
    let (glyph1, glyph2) = if !play_dropout {
        (prob.0, prob.0)
    } else if swap_glyphs {
        (prob.1, prob.0)
    } else {
        (prob.0, prob.1)
    };
    println!("----- Question: {}/{} -----", q + 1, q_len);
    flush();
    sleep(Duration::from_secs_f32(1.0)).await;
    println!("Glyph 1...");
    flush();
    play_and_wait(
        tty,
        &retime_eq_spaced(a_bet.get_other_glyph(glyph1), prob.2),
    )
    .await?;
    println!("Glyph 2...");
    flush();
    play_and_wait(
        tty,
        &retime_eq_spaced(a_bet.get_other_glyph(glyph2), prob.2),
    )
    .await?;
    let start = Instant::now();
    let answer = ask(
        "Did the device play the same pattern twice?\n(type 'y','n' or '?' if you're unsure, then [Enter]): ",
        &["y", "n", "?"]
    )?;
    let duration = Instant::now().duration_since(start);
    let unsure = answer == "?";
    let correct = ((answer == "y") ^ play_dropout) && !unsure;
    Ok(DropoutData {
        id: prob_id,
        glyph: prob.0.to_owned(),
        drop_glyph: prob.1.to_owned(),
        speed: prob.2,
        drop_played: play_dropout,
        duration_ms: duration.as_millis(),
        correct,
        unsure,
    })
}

async fn dropout_exp(
    mut out_writer: Writer<File>,
    mut tty: TTYPort,
    a_bet: &Alphabet,
) -> anyhow::Result<()> {
    clear_term();
    // TODO change this text
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
    calibrate_until_enter(&mut tty, a_bet).await?;

    // TODO add the rest of these
    let prob_pairs = vec![
        ("col0_up", "col0_up_dropout0"),
        ("col0_up", "col0_up_dropout1"),
        ("col2_down", "col2_down_dropout0"),
        ("col2_down", "col2_down_dropout1"),
        ("row0_left", "row0_left_dropout0"),
        ("row0_left", "row0_left_dropout1"),
        ("row0_left", "row0_left_dropout2"),
        ("row0_left", "row0_left_dropout3"),
        ("row0_left", "row0_left_dropout4"),
        ("row1_right", "row1_right_dropout0"),
        ("row1_right", "row1_right_dropout1"),
        ("row1_right", "row1_right_dropout2"),
        ("row1_right", "row1_right_dropout3"),
        ("row1_right", "row1_right_dropout4"),
        ("row2_right", "row2_right_dropout0"),
        ("row2_right", "row2_right_dropout1"),
        ("row2_right", "row2_right_dropout2"),
        ("row2_right", "row2_right_dropout3"),
        ("row2_right", "row2_right_dropout4"),
        ("clockwise", "clockwise_dropout0"),
        ("clockwise", "clockwise_dropout1"),
        ("clockwise", "clockwise_dropout2"),
        ("clockwise", "clockwise_dropout3"),
        ("clockwise", "clockwise_dropout4"),
        ("anticlockwise", "anticlockwise_dropout0"),
        ("anticlockwise", "anticlockwise_dropout1"),
        ("anticlockwise", "anticlockwise_dropout2"),
        ("anticlockwise", "anticlockwise_dropout3"),
        ("anticlockwise", "anticlockwise_dropout4"),
    ];
    let prob_len = prob_pairs.len();
    let speeds = iter::repeat(50)
        .take(prob_len)
        .chain(iter::repeat(150).take(prob_len));
    let mut problems: Vec<(usize, (&str, &str, u16))> = prob_pairs
        .into_iter()
        .cycle()
        .zip(speeds)
        .map(|((p1, p2), p3)| (p1, p2, p3))
        .enumerate()
        .collect();
    problems.shuffle(&mut rng());
    let q_len = problems.len();
    for (q, (p_id, prob)) in problems.into_iter().enumerate() {
        loop {
            clear_term();
            match dropout_problem(&mut tty, a_bet, prob, q, q_len, p_id).await {
                Ok(data) => {
                    out_writer.serialize(data)?;
                    out_writer.flush()?;
                    break;
                }
                Err(e) => {
                    println!("An error has occured on problem {}, {}", p_id, e);
                    let answer = ask(
                        "Would you like to retry this problem (otherwise, skip it)?[Y/n]: ",
                        &["y", "n", ""],
                    )?;
                    if answer == "n" {
                        out_writer.serialize(DropoutData {
                            id: p_id,
                            glyph: "error".to_owned(),
                            drop_glyph: "error".to_owned(),
                            speed: 0,
                            drop_played: false,
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

async fn alphabet_problem(
    tty: &mut TTYPort,
    a_bet: &Alphabet,
    prob: (char, u16),
    q: usize,
    q_len: usize,
    occurrence: usize,
) -> anyhow::Result<AlphabetData> {
    println!("----- Question: {}/{} -----", q + 1, q_len);
    flush();
    sleep(Duration::from_secs_f32(1.0)).await;
    println!("Playing glyph...");
    flush();
    play_and_wait(tty, &retime_eq_spaced(a_bet.get_glyph(prob.0), prob.1)).await?;
    let start = Instant::now();
    let options: Vec<String> = ('a'..='z')
        .chain(iter::once('?'))
        .map(|c| c.to_string())
        .collect();
    let answer = ask(
        "What letter just played?\n(type 'a', 'b', 'c', ..., 'z' or '?' if you're unsure, then [Enter]): ",
        &options
    )?;
    let duration = Instant::now().duration_since(start);
    let answer = answer.chars().nth(0).unwrap();
    let unsure = answer == '?';
    let correct = (answer == prob.0) && !unsure;
    if correct {
        println!("\nCorrect!");
    } else {
        println!("\nIncorrect, answer: '{}'", prob.0);
    }
    print!("Press [Enter] to continue: ");
    flush();
    io::stdin().read_line(&mut String::new())?;
    Ok(AlphabetData {
        c: prob.0,
        speed: prob.1,
        answer,
        duration_ms: duration.as_millis(),
        occurrence,
        correct,
        unsure,
    })
}

async fn alphabet_exp(
    mut out_writer: Writer<File>,
    mut tty: TTYPort,
    a_bet: &Alphabet,
) -> anyhow::Result<()> {
    clear_term();
    print!(
"In this experiment, a glyph will be played on the device and you will have to identify what letter of the alphabet has been played.
You will be shown what all of the glyphs are before you begin answering questions.

Press [Enter] when you're ready to begin:"
    );
    flush();
    calibrate_until_enter(&mut tty, a_bet).await?;

    'learn: for c in 'a'..='z' {
        clear_term();
        println!("----- Glyph '{}' -----", c);
        println_glyph(a_bet.get_glyph(c));
        flush();
        sleep(Duration::from_secs_f32(1.0)).await;
        let mut answer = String::new();
        while answer != "n" {
            println!("Playing...");
            flush();
            play_and_wait(&mut tty, &retime_eq_spaced(a_bet.get_glyph(c), 150)).await?;
            println!("Playing fast...");
            flush();
            play_and_wait(&mut tty, &retime_eq_spaced(a_bet.get_glyph(c), 30)).await?;
            answer = ask(
                "Would you like to replay this glyph (otherwise, advance to the next letter)?[Y/n]: ",
                &["y", "n", "skip", ""]
            )?;
            if answer == "skip" {
                break 'learn;
            }
        }
    }

    let problems = {
        let mut slow_chars: Vec<(char, u16)> = ('a'..='z').zip(iter::repeat(150)).collect();
        slow_chars.shuffle(&mut rng());
        let mut fast_chars: Vec<(char, u16)> = ('a'..='z').zip(iter::repeat(30)).collect();
        fast_chars.shuffle(&mut rng());
        slow_chars.append(&mut fast_chars);
        slow_chars
    };

    let q_len = problems.len();
    let mut occurrences = vec![0; 'z' as usize - 'a' as usize + 1];
    for (q, prob) in problems.into_iter().enumerate() {
        loop {
            clear_term();
            match alphabet_problem(
                &mut tty,
                a_bet,
                prob,
                q,
                q_len,
                occurrences[prob.0 as usize - 'a' as usize],
            )
            .await
            {
                Ok(data) => {
                    occurrences[prob.0 as usize - 'a' as usize] += 1;
                    out_writer.serialize(data)?;
                    break;
                }
                Err(e) => {
                    println!("An error has occured on problem {}, {}", prob.0, e);
                    let answer = ask(
                        "Would you like to retry this problem (otherwise, skip it)?[Y/n]: ",
                        &["y", "n", ""],
                    )?;
                    if answer == "n" {
                        out_writer.serialize(AlphabetData {
                            c: '%',
                            speed: 0,
                            answer: ' ',
                            duration_ms: 0,
                            occurrence: 0,
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

async fn draw_problem(
    tty: &mut TTYPort,
    a_bet: &Alphabet,
    prob: (char, u16),
    q: usize,
    q_len: usize,
) -> anyhow::Result<DrawData> {
    println!("----- Question: {}/{} -----", q + 1, q_len);
    flush();
    sleep(Duration::from_secs_f32(1.0)).await;
    println!("Playing glyph...");
    flush();
    play_and_wait(tty, &retime_eq_spaced(a_bet.get_glyph(prob.0), prob.1)).await?;
    let start = Instant::now();
    let answer = ask(
        "Please draw the glyph you just felt, then rate how \"pathy\" felt it was from 1 to 5.\n(type '1', '2', '3', '4' or '5', then [Enter]): ",
        &["1", "2", "3", "4", "5"],
    )?;
    let duration = Instant::now().duration_since(start);
    Ok(DrawData {
        glyph: prob.0,
        speed: prob.1,
        duration_ms: duration.as_millis(),
        pathiness: answer.parse()?,
    })
}

async fn draw_exp(
    mut out_writer: Writer<File>,
    mut tty: TTYPort,
    a_bet: &Alphabet,
) -> anyhow::Result<()> {
    clear_term();
    print!(
"In this experiment, for each question, the device will vibrate the motors in a \"path\".

You will be asked to draw this path with a pencil and paper, and type a rating out of 5 of how \"pathy\" the pattern feels.

Press [Enter] when you're ready to begin:"
    );
    flush();
    calibrate_until_enter(&mut tty, a_bet).await?;

    let abet_len = 'z' as usize - 'a' as usize + 1;
    let speeds = iter::repeat(30)
        .take(abet_len / 2)
        .chain(iter::repeat(50).take(abet_len / 2))
        .chain(iter::repeat(70).take(abet_len / 2))
        .chain(iter::repeat(100).take(abet_len / 2))
        .chain(iter::repeat(150).take(abet_len / 2))
        .chain(iter::repeat(250).take(abet_len / 2));
    let mut abet_chars: Vec<char> = ('a'..='z').collect();
    abet_chars.shuffle(&mut rng());
    let chars = abet_chars.into_iter().cycle().take(abet_len * 3);
    let mut problems: Vec<(char, u16)> = chars.zip(speeds).collect();
    problems.shuffle(&mut rng());

    let q_len = problems.len();
    for (q, prob) in problems.into_iter().enumerate() {
        loop {
            clear_term();
            match draw_problem(&mut tty, a_bet, prob, q, q_len).await {
                Ok(data) => {
                    out_writer.serialize(data)?;
                    out_writer.flush()?;
                    break;
                }
                Err(e) => {
                    println!("An error has occured on problem {}, {}", q, e);
                    let answer = ask(
                        "Would you like to retry this problem (otherwise, skip it)?[Y/n]: ",
                        &["y", "n", ""],
                    )?;
                    if answer == "n" {
                        out_writer.serialize(DrawData {
                            glyph: '?',
                            speed: 0,
                            duration_ms: 0,
                            pathiness: 0,
                        })?;
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if Path::exists(&cli.out_path) && cli.out_path != PathBuf::from("/dev/null") {
        return Err(anyhow!("OUTPUT_FILE path already exists"));
    }

    let tty = TTYPort::open(&serialport::new(cli.tty_path.to_string_lossy(), 115200))?;
    let out_writer = csv::WriterBuilder::new().from_path(cli.out_path)?;

    let alphabets = init_alphabets();

    match cli.exp {
        Exp::Dropout => dropout_exp(out_writer, tty, alphabets.get("distinguish").unwrap()).await,
        Exp::Alphabet => alphabet_exp(out_writer, tty, alphabets.get("roud_graff").unwrap()).await,
        Exp::Draw => draw_exp(out_writer, tty, alphabets.get("roud_graff").unwrap()).await,
    }?;

    Ok(())
}
