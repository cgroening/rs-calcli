/// Display format for numbers
#[derive(Clone, Copy, PartialEq)]
pub enum DisplayFormat {
    Normal,
    Scientific,
}

/// Result of executing a command
pub enum CommandResult {
    Quit,
    Help,
    FormatChanged(String), // Message about format change
}

/// Handles calculator commands and number formatting
pub struct CommandHandler {
    display_format: DisplayFormat,
    decimal_places: usize,
}

impl CommandHandler {
    /// Creates a new CommandHandler with default settings
    pub fn new() -> Self {
        CommandHandler {
            display_format: DisplayFormat::Normal,
            decimal_places: 3,
        }
    }

    /// Checks if the input is a command (starts with ':')
    pub fn is_command(input: &str) -> bool {
        input.starts_with(':')
    }

    /// Executes a command and returns the result
    ///
    /// # Arguments
    /// - `input`: Command string (e.g., ":q", ":d", ":d5", ":s2")
    ///
    /// # Returns
    /// - `Result<CommandResult, String>`: Command result or error message
    pub fn execute_command(&mut self, input: &str) -> Result<CommandResult, String> {
        // Handle :q (quit)
        if input.eq_ignore_ascii_case(":q") {
            return Ok(CommandResult::Quit);
        }

        // Handle :h (help)
        if input == ":h" {
            return Ok(CommandResult::Help);
        }

        // Handle :d (decimal/normal notation)
        if input.starts_with(":d") {
            let decimal_str = &input[2..];
            let decimals = if decimal_str.is_empty() {
                3 // Default
            } else {
                decimal_str.parse::<usize>()
                    .map_err(|_| format!("Invalid decimal count: '{}'", decimal_str))?
            };

            self.display_format = DisplayFormat::Normal;
            self.decimal_places = decimals;
            return Ok(CommandResult::FormatChanged(
                format!("Set to normal notation with {} decimal places", decimals)
            ));
        }

        // Handle :s (scientific notation)
        if input.starts_with(":s") {
            let decimal_str = &input[2..];
            let decimals = if decimal_str.is_empty() {
                3 // Default
            } else {
                decimal_str.parse::<usize>()
                    .map_err(|_| format!("Invalid decimal count: '{}'", decimal_str))?
            };

            self.display_format = DisplayFormat::Scientific;
            self.decimal_places = decimals;
            return Ok(CommandResult::FormatChanged(
                format!("Set to scientific notation with {} decimal places", decimals)
            ));
        }

        Err(format!("Unknown command: '{}'", input))
    }

    /// Formats a number according to current display settings
    ///
    /// # Arguments
    /// - `value`: Number to format
    ///
    /// # Returns
    /// - `String`: Formatted number
    pub fn format_number(&self, value: f64) -> String {
        match self.display_format {
            DisplayFormat::Normal => {
                format!("{:.prec$}", value, prec = self.decimal_places)
            }
            DisplayFormat::Scientific => {
                format!("{:.prec$e}", value, prec = self.decimal_places)
            }
        }
    }
}
