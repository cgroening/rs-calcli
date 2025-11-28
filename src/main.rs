use std::sync::{Arc, Mutex};

// https://crates.io/crates/colored
use colored::Colorize;

// https://crates.io/crates/rustyline
// use rustyline::error::ReadlineError;
// use rustyline::Editor;
// use rustyline::history::DefaultHistory;


use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline::history::DefaultHistory;

// https://crates.io/crates/meval
use meval::eval_str;


fn main() {
    let mut rl = Editor::<(), DefaultHistory>::new().expect("Konnte rustyline nicht initialisieren");



    // println!("Welcome to {} – the calculator for the CLI.", "calcli".blue());
    print!("{}", "Welcome to ".bold());
    print!("{}", "calcli".blue().bold());
    println!("{}", " – the calulator for the command line".bold());
    println!("Enter a mathematical expression to evaluate it or {} for more \
              information.", "help".italic());

    loop {

        let readline = rl.readline(&format!("{}", "[calcli] > ".to_string().magenta().bold()));



        match readline {
            Ok(line) => {
                let input = line.trim();

                if input.eq_ignore_ascii_case("exit") {
                    println!("Auf Wiedersehen!");
                    break;
                }

                // test
                if input.contains("XXX") {
                    println!("XXX netered!");
                    continue;
                }

                if input.is_empty() {
                    continue;
                }

                // rl.add_history_entry(input).unwrap_or(());
                let _ = rl.add_history_entry(input);
                // rl.add_history_entry(input).unwrap();

                match eval_str(input) {
                    Ok(result) => println!("= {}", result.to_string().green().bold()),
                    Err(e) => eprintln!("{}", e.to_string().red().bold()),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("(CTRL-C) Zum Beenden 'exit' eingeben.");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("\n(CTRL-D) Beendet.");
                break;
            }
            Err(err) => {
                eprintln!("Fehler: {:?}", err);
                break;
            }
        }
    }
}