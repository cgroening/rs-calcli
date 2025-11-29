// https://crates.io/crates/meval
use meval::eval_str;


pub struct Parser {
    // Make ans optional to be able to represent "no previous answer"
    ans: Option<f64>,
}

impl Parser {
    pub fn new() -> Self {
        Parser { ans: None }
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
            if let Some(answer) = self.ans {
                format!("ans{}", input)
            } else {
                return Err("No previous answer available for 'ans'".to_string());
            }
        } else {
            input.to_string()
        };

        // User , as decimal separator, replace with .
        let input = input.replace(',', ".");

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

        // Evaluate input with meval
        let result = eval_str(&parsed_input)
            .map_err(|e| e.to_string())?;

        // Update previous answer
        self.ans = Some(result);

        // Return result
        Ok(result.to_string())

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