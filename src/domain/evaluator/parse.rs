//! Reading an expression into a tree: a tokenizer over byte offsets and a
//! recursive-descent parser.
//!
//! The grammar, loosest binding first:
//!
//! ```text
//! sum     = product (("+" | "-") product)*
//! product = signed (("*" | "/" | "%") signed)*
//! signed  = ("+" | "-") signed | power
//! power   = atom ("^" signed)?          right-associative: 2^3^2 = 2^9
//! atom    = number | name | name "(" sum ("," sum)* ")" | "(" sum ")"
//! ```
//!
//! A sign binds looser than `^`, so `-2^2` is `-4`, and an exponent may carry
//! one (`2^-1`). The error texts are the ones calcli showed while `meval`
//! evaluated, so the status line and the stored history read the same.

/// A parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Expr {
    /// A number literal.
    Number(f64),
    /// A name: a constant, or an unknown variable.
    Name(String),
    /// A sign in front of an operand.
    Negate(Box<Expr>),
    /// A binary operation.
    Binary(Op, Box<Expr>, Box<Expr>),
    /// A function call with its arguments.
    Call(String, Vec<Expr>),
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

/// One token and the byte it starts at.
#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: Kind,
    at: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Number(f64),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Open,
    Close,
    Comma,
    /// Something no token starts with (`!`, `.5`, `1e`).
    Invalid,
}

/// Parses `input` into a tree.
///
/// # Errors
///
/// Returns the message of the first problem: a missing operand at the end,
/// an unexpected token (with its byte offset) or missing closing
/// parentheses.
pub(super) fn parse(input: &str) -> Result<Expr, String> {
    let tokens = tokenize(input);
    let mut parser = Parser {
        tokens,
        next: 0,
        depth: 0,
    };
    let expr = parser.sum()?;
    match parser.peek() {
        None => Ok(expr),
        Some(token) => Err(unexpected(token.at)),
    }
}

/// The message for a token that cannot stand where it does.
fn unexpected(at: usize) -> String {
    format!("Parse error: Unexpected token at byte {at}.")
}

/// The message for an expression that ends where an operand is due.
const MISSING_ARGUMENT: &str =
    "Parse error: Missing argument at the end of expression.";

/// The message for `count` parentheses left open at the end.
fn missing_parens(count: usize) -> String {
    let noun = if count == 1 {
        "parenthesis"
    } else {
        "parentheses"
    };
    format!("Parse error: Missing {count} right {noun}.")
}

/// Splits `input` into tokens, skipping whitespace.
fn tokenize(input: &str) -> Vec<Token> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            at += 1;
            continue;
        }
        let start = at;
        let kind = match byte {
            b'+' => Kind::Plus,
            b'-' => Kind::Minus,
            b'*' => Kind::Star,
            b'/' => Kind::Slash,
            b'%' => Kind::Percent,
            b'^' => Kind::Caret,
            b'(' => Kind::Open,
            b')' => Kind::Close,
            b',' => Kind::Comma,
            b'0'..=b'9' => {
                let (kind, end) = number(input, start);
                at = end;
                tokens.push(Token { kind, at: start });
                continue;
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                let end = bytes[start..]
                    .iter()
                    .position(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
                    .map_or(bytes.len(), |len| start + len);
                at = end;
                tokens.push(Token {
                    kind: Kind::Name(input[start..end].to_string()),
                    at: start,
                });
                continue;
            }
            _ => Kind::Invalid,
        };
        tokens.push(Token { kind, at: start });
        // An invalid byte ends the input: nothing after it is read.
        if kind_is_invalid(&tokens) {
            break;
        }
        at += 1;
    }
    tokens
}

/// Whether the last token pushed is invalid.
fn kind_is_invalid(tokens: &[Token]) -> bool {
    tokens.last().is_some_and(|t| t.kind == Kind::Invalid)
}

/// Reads the number starting at byte `start` of `input`: digits, an
/// optional `.` with optional digits, an optional exponent (`e`/`E`, an
/// optional sign, at least one digit). A dangling exponent (`1e`) makes the
/// whole number invalid. Returns the token and the byte after it.
fn number(input: &str, start: usize) -> (Kind, usize) {
    let bytes = input.as_bytes();
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .position(|b| !b.is_ascii_digit())
            .map_or(bytes.len(), |len| from + len)
    };
    let mut end = digits(start);
    if bytes.get(end) == Some(&b'.') {
        end = digits(end + 1);
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let after = digits(exponent);
        if after == exponent {
            return (Kind::Invalid, bytes.len());
        }
        end = after;
    }
    match input[start..end].parse::<f64>() {
        Ok(value) => (Kind::Number(value), end),
        Err(_) => (Kind::Invalid, bytes.len()),
    }
}

/// The parser's position in the tokens.
struct Parser {
    tokens: Vec<Token>,
    next: usize,
    /// How many `(` are open at the current position.
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next)
    }

    /// Takes the next token when it is `kind`.
    fn eat(&mut self, kind: &Kind) -> bool {
        if self.peek().is_some_and(|t| t.kind == *kind) {
            self.next += 1;
            true
        } else {
            false
        }
    }

    fn sum(&mut self) -> Result<Expr, String> {
        let mut left = self.product()?;
        loop {
            let op = if self.eat(&Kind::Plus) {
                Op::Add
            } else if self.eat(&Kind::Minus) {
                Op::Sub
            } else {
                return Ok(left);
            };
            let right = self.product()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn product(&mut self) -> Result<Expr, String> {
        let mut left = self.signed()?;
        loop {
            let op = if self.eat(&Kind::Star) {
                Op::Mul
            } else if self.eat(&Kind::Slash) {
                Op::Div
            } else if self.eat(&Kind::Percent) {
                Op::Rem
            } else {
                return Ok(left);
            };
            let right = self.signed()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn signed(&mut self) -> Result<Expr, String> {
        if self.eat(&Kind::Plus) {
            return self.signed();
        }
        if self.eat(&Kind::Minus) {
            return Ok(Expr::Negate(Box::new(self.signed()?)));
        }
        self.power()
    }

    fn power(&mut self) -> Result<Expr, String> {
        let base = self.atom()?;
        if self.eat(&Kind::Caret) {
            let exponent = self.signed()?;
            return Ok(Expr::Binary(
                Op::Pow,
                Box::new(base),
                Box::new(exponent),
            ));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<Expr, String> {
        let Some(token) = self.peek().cloned() else {
            return Err(MISSING_ARGUMENT.to_string());
        };
        self.next += 1;
        match token.kind {
            Kind::Number(value) => Ok(Expr::Number(value)),
            Kind::Name(name) if self.eat(&Kind::Open) => {
                self.depth += 1;
                let args = self.arguments()?;
                Ok(Expr::Call(name, args))
            }
            Kind::Name(name) => Ok(Expr::Name(name)),
            Kind::Open => {
                self.depth += 1;
                let inner = self.sum()?;
                self.close()?;
                Ok(inner)
            }
            _ => Err(unexpected(token.at)),
        }
    }

    /// The arguments of a call after its `(`, through the closing `)`.
    fn arguments(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = vec![self.sum()?];
        while self.eat(&Kind::Comma) {
            args.push(self.sum()?);
        }
        self.close()?;
        Ok(args)
    }

    /// Takes the `)` that closes the innermost open parenthesis.
    fn close(&mut self) -> Result<(), String> {
        match self.peek() {
            Some(token) if token.kind == Kind::Close => {
                self.next += 1;
                self.depth -= 1;
                Ok(())
            }
            Some(token) => Err(unexpected(token.at)),
            None => Err(missing_parens(self.depth)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_signs_build_the_expected_tree() {
        let tree = parse("-2^2").unwrap();
        assert!(matches!(tree, Expr::Negate(_)), "{tree:?}");
        let tree = parse("2^3^2").unwrap();
        let Expr::Binary(Op::Pow, _, right) = tree else {
            panic!("a power");
        };
        assert!(matches!(*right, Expr::Binary(Op::Pow, _, _)));
    }

    #[test]
    fn numbers_read_like_before() {
        assert_eq!(parse("5.").unwrap(), Expr::Number(5.0));
        assert_eq!(parse("1e+3").unwrap(), Expr::Number(1000.0));
        assert_eq!(parse(".5"), Err(unexpected(0)));
        assert_eq!(parse("1e"), Err(unexpected(0)));
    }
}
