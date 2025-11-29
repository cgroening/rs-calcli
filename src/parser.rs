use std::collections::HashMap;
use regex::Regex;
// https://crates.io/crates/meval
use meval::eval_str;



/// Parser for mathematical expressions
///
/// Keeps track of previous answer and variables
///
/// # Attributes
/// - `ans`: Previous answer (`Optional` to represent "no previous answer")
/// - `vars`: Variables saved by the user
pub struct Parser {
    /// Previous answer (`Optional` to represent "no previous answer")
    pub ans: Option<f64>,

    /// Variables saved by the user
    vars: HashMap<String, f64>,
}

impl Parser {
    pub fn new() -> Self {
        Parser { ans: None, vars: HashMap::new() }
    }

    /// Handles parsing and evaluating a mathematical expression
    ///
    /// Reference to previous answer is done via "ans" keyword or by starting
    /// the expression with an operator (+, -, *, /). Variables can be saved by
    /// starting the input with `=varname` (e.g. `=x` to save the previous
    /// answer to a variable) or by using variable assignment (e.g. `x=5+1`).
    ///
    /// # Arguments
    /// - `input`: Mathematical expression as a string
    ///
    /// # Returns
    /// - `Result<String, String>`: Result of evaluation as a string
    ///   or error message
    pub fn parse(&mut self, input: &str) -> Result<String, String> {
        // Parse input step by step
        let input = self.handle_start_with_operator(input);
        if self.handle_start_with_equal_sign(&input).is_ok() {
            return self.handle_start_with_equal_sign(&input);
        }
        let input = self.handle_comma_and_semicolon(&input);
        let input = self.handle_ans_reference(&input);
        let input = self.replace_vars_in_input(&input);

        if let Some(var_assignment_result) = self.handle_variable_assignment(&input)? {
            return Ok(var_assignment_result);
        }

        // Evaluate input with meval, update previous answer and return result
        let result = eval_str(&input).map_err(|e| e.to_string())?;
        self.ans = Some(result);
        Ok("= ".to_string() + &result.to_string())

    }

    /// Handles the start of the input with an operator
    ///
    /// # Arguments
    /// - `input`: Mathematical expression as a string
    ///
    /// # Returns
    /// - `String`: Modified input with "ans" prepended if necessary
    fn handle_start_with_operator(&self, input: &str) -> String {
        if input.starts_with('+')
            || input.starts_with('-')
            || input.starts_with('*')
            || input.starts_with('/')
        {
            if let Some(_answer) = self.ans {
                format!("ans{}", input)
            } else {
                input.to_string()
            }
        } else {
            input.to_string()
        }
    }

    /// Handles the start of the input with '=' to save previous answer
    /// to a variable.
    ///
    /// # Arguments
    /// - `input`: Mathematical expression as a string
    ///
    /// # Returns
    /// - `Result<String, String>`: Success message or error message
    fn handle_start_with_equal_sign(&mut self, input: &str)
    -> Result<String, String> {
        if input.starts_with('=') {
            let var_name = input[1..].trim();
            if let Some(answer) = self.ans {
                self.vars.insert(var_name.to_string(), answer);
                return Ok(format!("Saved answer to variable '{}'", var_name));
            } else {
                return Err("No previous answer available to save".to_string());
            }
        }
        Err("Input does not start with '='".to_string())
    }

    /// Handles replacement of commas and semicolons in the input
    ///
    /// Commas are replaced with dots and semicolons with commas.
    ///
    /// # Arguments
    /// - `input`: Mathematical expression as a string
    ///
    /// # Returns
    /// - `String`: Modified input with replacements
    fn handle_comma_and_semicolon(&self, input: &str) -> String {
        let input = input.replace(',', ".");
        let input = input.replace(';', ",");
        input
    }

    /// Handles reference to previous answer in the input
    ///
    /// # Arguments
    /// - `input`: Mathematical expression as a string
    ///
    /// # Returns
    /// - `String`: Modified input with "ans" replaced by previous answer
    fn handle_ans_reference(&self, input: &str) -> String {
        if input.contains("ans") {
            if let Some(answer) = self.ans {
                input.replace("ans", &answer.to_string())
            } else {
                input.to_string()
            }
        } else {
            input.to_string()
        }
    }

    /// Replaces variables in the input with their values
    ///
    /// # Arguments
    /// - `input`: Mathematical expression as a string
    ///
    /// # Returns
    /// - `String`: Modified input with variables replaced by their values
    fn replace_vars_in_input(&self, input: &str) -> String {
        self.vars.iter().fold(input.to_string(), |acc, (key, value)| {
            // Create regex pattern that matches the variable name with word boundaries
            let pattern = format!(r"\b{}\b", regex::escape(key));
            let re = Regex::new(&pattern).unwrap();
            re.replace_all(&acc, value.to_string()).to_string()
        })
    }

    /// Handles variable assignment in the input
    ///
    /// If the input begings with `varname=` it is treated as a variable
    /// assignment (e.g. x=5+1 or y=ans+2). The expression is evaluated and the
    /// results saved in the HashMap of variables.
    /// The result is returned as a string.
    ///
    /// # Arguments
    /// - `input`: Mathematical expression as a string
    ///
    /// # Returns
    /// - `Result<Option<String>, String>`: Result of evaluation as a string
    ///   or error message. Returns `Ok(None)` if no variable assignment
    ///   is found.
    ///
    /// If a variable assignment is found, returns `Ok(Some(result_string))`.
    ///
    /// # Example
    /// - Input: "x=5+1"
    /// - Output: Ok(Some("x = 6"))
    ///
    /// If no variable assignment is found, returns Ok(None).
    fn handle_variable_assignment(&mut self, input: &str)
    -> Result<Option<String>, String> {
        if let Some(eq_pos) = input.find('=') {
            let var_name = input[..eq_pos].trim();
            let expression = input[eq_pos + 1..].trim();

            // Validate variable name
            if var_name.is_empty() || !var_name.chars().all(
                |c| c.is_alphanumeric() || c == '_'
            ) {
                return Err(format!("Invalid variable name: '{}'", var_name));
            }

            // Process expression: replace ans and variables
            let expression = self.handle_ans_reference(expression);
            let expression = self.replace_vars_in_input(&expression);

            // Evaluate the expression
            let result = eval_str(&expression).map_err(|e| e.to_string())?;

            // Save result to variable, update previous answer
            self.vars.insert(var_name.to_string(), result);
            self.ans = Some(result);

            // Return the variable name and result
            return Ok(Some(format!("{} = {}", var_name, result)));
        }
        Ok(None)
    }
}