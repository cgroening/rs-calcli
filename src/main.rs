// https://crates.io/crates/colored
use colored::Colorize;

// https://crates.io/crates/rustyline
use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline::history::DefaultHistory;

// https://crates.io/crates/meval
use meval::eval_str;


fn main() {
    let mut rl = Editor::<(), DefaultHistory>::new().expect(
        "rustyline could not be initialized"
    );

    // Print app name
    print!("{}", "calcli".blue().bold());
    println!("{}", " – calulator for the command line".bold());
    println!("Enter a mathematical expression to evaluate it or {} for more \
              information.", "help".italic());

    // Start repl (read-eval-print loop)
    loop {
        let readline = rl.readline(
            &format!("{}", "[calcli] > ".to_string().magenta().bold())
        );

        match readline {
            Ok(line) => {
                let input = line.trim();

                if input.eq_ignore_ascii_case(".q") {
                    println!("Exiting ...");
                    break;
                }

                // test
                if input.contains("XXX") {
                    println!("XXX entered!");
                    continue;
                }

                if input.is_empty() {
                    continue;
                }

                // Add input to history
                let _ = rl.add_history_entry(input);

                // Evaluate input
                match eval_str(input) {
                    Ok(result) =>
                        println!("= {}", result.to_string().green().bold()),
                    Err(e) =>
                        eprintln!("{}", e.to_string().red().bold()),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("(CTRL-C) Exiting...");
                break;
                // continue;
            }
            Err(ReadlineError::Eof) => {
                println!("\n(CTRL-D) Exiting...");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }
}