use std::collections::HashMap;

// https://crates.io/crates/meval
use meval::eval_str;


pub struct Parser {
    /// Previous answer (optional to represent "no previous answer")
    pub ans: Option<f64>,

    /// Variables saved by the user
    vars: HashMap<String, f64>,
}

impl Parser {
    pub fn new() -> Self {
        Parser { ans: None, vars: HashMap::new() }
    }

    /// Parse input string with meval, replacing "ans" with the previous answer
    /// if available. If the expression containts "ans" but no previous answer
    /// is avvailable in error is returned.
    pub fn parse(&mut self, input: &str) -> Result<String, String> {
        // If the input begins with an operator (+, -, * or /) prepend the
        // prepend "ans" to the input
        let input = if input.starts_with('+')
            || input.starts_with('-')
            || input.starts_with('*')
            || input.starts_with('/')
        {
            if let Some(_answer) = self.ans {
                format!("ans{}", input)
            } else {
                return Err("No previous answer available for 'ans'".to_string());
            }
        } else {
            input.to_string()
        };

        // If the input begins with = save the previous answer to a variable
        // the name is the rest of the input after the =
        if input.starts_with('=') {
            let var_name = input[1..].trim();
            if let Some(answer) = self.ans {
                self.vars.insert(var_name.to_string(), answer);
                return Ok(format!("Saved answer to variable '{}'", var_name));
            } else {
                return Err("No previous answer available to save".to_string());
            }
        }





        // Use , as decimal separator, replace with .
        let input = input.replace(',', ".");

        // Replace ; with ,
        let input = input.replace(';', ",");

        // Replace "ans" with previous answer if available
        let parsed_input = if input.contains("ans") {
            if let Some(answer) = self.ans {
                input.replace("ans", &answer.to_string())
            } else {
                return Err("No previous answer available for 'ans'".to_string());
            }
        } else {
            input.to_string()
        };



        // Replace variables in the input with their values
        let parsed_input = self.vars.iter().fold(parsed_input, |acc, (key, value)| {
            acc.replace(key, &value.to_string())
        });




        // // Evaluate input with meval
        // match eval_str(&parsed_input) {
        //     Ok(result) => {
        //         // Update previous answer
        //         self.ans = Some(result);

        //         // Return result as string
        //         Ok(result.to_string())
        //     }
        //     Err(e) => Err(e.to_string()),
        // }

        // // Save result as previous answer

        // // Return result


        // If the input begings with varname= treat it as variable assignment
        // (e.g. x=5+1 or y=ans+2). Evaluate the expression, save the result
        // to the variable and return the result.
        // Check if input contains variable assignment (varname=expression)
        if let Some(eq_pos) = input.find('=') {
            let var_name = input[..eq_pos].trim();
            let expression = input[eq_pos + 1..].trim();

            // Validate variable name (simple check: non-empty and alphanumeric)
            if var_name.is_empty() || !var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(format!("Invalid variable name: '{}'", var_name));
            }

            // Process the expression part
            let expression = expression.replace(',', ".");
            let expression = expression.replace(';', ",");

            // Replace "ans" with previous answer if available
            let parsed_expr = if expression.contains("ans") {
                if let Some(answer) = self.ans {
                    expression.replace("ans", &answer.to_string())
                } else {
                    return Err("No previous answer available for 'ans'".to_string());
                }
            } else {
                expression.to_string()
            };

            // Replace variables in the expression with their values
            let parsed_expr = self.vars.iter().fold(parsed_expr, |acc, (key, value)| {
                acc.replace(key, &value.to_string())
            });

            // Evaluate the expression
            let result = eval_str(&parsed_expr)
                .map_err(|e| e.to_string())?;

            // Save result to variable
            self.vars.insert(var_name.to_string(), result);

            // Update previous answer
            self.ans = Some(result);

            // Return result with variable name
            return Ok(format!("{} = {}", var_name, result));
        }







        // Evaluate input with meval
        let result = eval_str(&parsed_input)
            .map_err(|e| e.to_string())?;

        // Update previous answer
        self.ans = Some(result);

        // Return result
            Ok("= ".to_string() + &result.to_string())

    }



    // pub fn parse(&self, input: &str) -> String {
    //     if let Some(answer) = self.ans {
    //         input.replace("ans", &answer.to_string())
    //     } else {
    //         input.to_string()
    //     }
    // }

    // pub fn set_answer(&mut self, answer: f64) {
    //     self.ans = Some(answer);
    // }

    // pub fn get_answer(&self) -> Option<f64> {
    //     self.ans
    // }
}