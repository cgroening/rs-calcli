// https://crates.io/crates/colored
use colored::Colorize;

// https://crates.io/crates/rustyline
use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline::history::DefaultHistory;

mod parser;
use parser::Parser;

mod commands;
use commands::{CommandHandler, CommandResult};


fn main() {
    let mut rl = Editor::<(), DefaultHistory>::new().expect(
        "rustyline could not be initialized"
    );

    // Print app name
    print!("{}", "calcli".blue().bold());
    println!("{}", " – calulator for the command line".bold());
    println!("Enter a mathematical expression to evaluate it or {} for more \
              information.\n", "help".italic());

    // Initialize parser and command handler
    let mut parser = Parser::new();
    let mut cmd_handler = CommandHandler::new();

    // Start repl (read-eval-print loop)
    loop {
        let readline = rl.readline(
            &format!("{}", ">>> ".to_string().magenta().bold())
        );

        match readline {
            Ok(line) => {
                let input = line.trim();

                // Ignore empty input
                if input.is_empty() {
                    continue;
                }

                // Add input to history
                let _ = rl.add_history_entry(input);

                // Check if input is a command
                if CommandHandler::is_command(input) {
                    match cmd_handler.execute_command(input) {
                        Ok(CommandResult::Quit) => {
                            println!("{}", "Exiting".blue().bold());
                            break;
                        }
                        Ok(CommandResult::Help) => {
                            println!(
                                "{}",
                                "Help information not yet implemented.".yellow().bold()
                            );
                        }
                        Ok(CommandResult::FormatChanged(msg)) => {
                            println!("{}", msg.blue().bold());
                        }
                        Err(e) => {
                            eprintln!("{}", e.red().bold());
                        }
                    }
                } else {
                    // Evaluate input with parser
                    match parser.parse(input, &cmd_handler) {
                        Ok(result) =>
                            println!("{}", result.green().bold()),
                        Err(e) =>
                            eprintln!("{}", e.red().bold()),
                    }
                }
            }

            // Handle CTRL-C and CTRL-D
            Err(ReadlineError::Interrupted) => {
                println!(
                    "{}",
                    "(CTRL-C) Copying result to clipboard not yet implemented."
                    .yellow().bold()
                );
                // TODO: Copy the last result to the clipboard
                // let result = parser.ans;
                // ...
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "\n(CTRL-D) Exiting...".yellow().bold());
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }
}
