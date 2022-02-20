use std::iter::Peekable;

use anyhow::bail;

#[derive(Debug)]
pub enum Token<'a> {
    Comment(&'a str),
    Param,
    Ident(&'a str),
    Colon,
    SemiColon,
    StringLiteral(&'a str),
    NumberLiteral(&'a str),
    Cell,
    QuestionMark,
    Mul,
    Equal,
    Add,
    Sub,
    Div,
    ParOpen,
    ParClose,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

pub fn lex(input: &'_ str) -> Result<Vec<Token<'_>>, anyhow::Error> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().enumerate().peekable().into_iter();

    macro_rules! add_single_char_token {
        ($token:ident) => {{
            chars.next();
            tokens.push(Token::$token);
        }};
    }

    while let Some(x) = chars.peek() {
        let (i, c) = *x; // drop mutable borrow
        match c {
            '#' => lex_comment(input, &mut tokens, &mut chars, i)?,
            c if c.is_whitespace() => {
                // skip whitespace here
                chars.next();
            }
            '(' => add_single_char_token!(ParOpen),
            ')' => add_single_char_token!(ParClose),
            ';' => add_single_char_token!(SemiColon),
            ':' => add_single_char_token!(Colon),
            '?' => add_single_char_token!(QuestionMark),
            '*' => add_single_char_token!(Mul),
            '+' => {
                chars.next();
                if let Some((_, c)) = chars.peek() {
                    if c.is_numeric() {
                        lex_number(input, &mut tokens, &mut chars, i, Sign::Positive)?
                    } else {
                        add_single_char_token!(Add);
                    }
                } else {
                    add_single_char_token!(Add);
                }
            }
            '-' => {
                chars.next();
                if let Some((_, c)) = chars.peek() {
                    if c.is_numeric() {
                        lex_number(input, &mut tokens, &mut chars, i, Sign::Negative)?
                    } else {
                        add_single_char_token!(Sub);
                    }
                } else {
                    add_single_char_token!(Sub);
                }
            }
            '/' => add_single_char_token!(Div),
            '=' | '>' | '<' => lex_operator(&mut tokens, &mut chars)?,
            '\"' => lex_string(input, &mut tokens, &mut chars, i)?,
            c => {
                if c.is_numeric() {
                    lex_number(input, &mut tokens, &mut chars, i, Sign::Unknown)?
                } else {
                    lex_ident(input, &mut tokens, &mut chars, i)?
                }
            }
        }
    }

    Ok(tokens)
}

type State = (usize, char);

enum Sign {
    Positive,
    Negative,
    Unknown,
}


/// FIXME: this function consumes one extra character
fn lex_number<'a, I: Iterator<Item = State>>(
    input: &'a str,
    tokens: &mut Vec<Token<'a>>,
    chars: &mut Peekable<I>,
    last_i: usize,
    sign: Sign,
) -> Result<(), anyhow::Error> {
    let mut number_str = String::with_capacity(6);
    if matches!(sign, Sign::Negative) {
        number_str.push('-');
    }
    let mut offset = 0;
    while let Some((_, c)) = chars.next() {
        offset += 1;
        number_str.push(c);
        let result: Result<f64, _> = number_str.parse();
        if result.is_err() {
            break;
        }
    }
    let ident = match sign {
        Sign::Positive | Sign::Negative => &input[last_i..last_i + offset],
        Sign::Unknown => &input[last_i..last_i + offset - 1],
    };
    let token = Token::NumberLiteral(ident);
    tokens.push(token);
    Ok(())
}

fn lex_string<'a, I: Iterator<Item = State>>(
    input: &'a str,
    tokens: &mut Vec<Token<'a>>,
    chars: &mut Peekable<I>,
    last_i: usize,
) -> Result<(), anyhow::Error> {
    let mut offset = 0;
    let mut string_closed = false;
    while chars.next().is_some() {
        offset += 1;
        if let Some((_, next_c)) = chars.peek() {
            if *next_c == '\"' {
                chars.next();
                string_closed = true;
                break;
            }
        }
    }
    if !string_closed {
        bail!("string literal opened but never closed");
    }
    let ident = &input[last_i + 1..last_i + offset];
    let token = Token::StringLiteral(ident);
    tokens.push(token);
    Ok(())
}

fn lex_operator<'a, I: Iterator<Item = State>>(
    tokens: &mut Vec<Token<'a>>,
    chars: &mut Peekable<I>,
) -> Result<(), anyhow::Error> {
    let (_, a) = chars.next().unwrap();

    let token = match a {
        '=' => {
            if let Some((_, next)) = chars.peek() {
                if *next == '=' {
                    chars.next();
                    Token::Equal
                } else {
                    bail!("expected char = bot got {}", next);
                }
            } else {
                bail!("expected char = bot got EOF");
            }
        }
        '>' => match chars.peek() {
            Some((_, '=')) => {
                chars.next();
                Token::GreaterThanOrEqual
            }
            _ => Token::GreaterThan,
        },
        '<' => match chars.peek() {
            Some((_, '=')) => {
                chars.next();
                Token::LessThanOrEqual
            }
            _ => Token::LessThan,
        },
        _ => bail!("unreachable"),
    };
    tokens.push(token);
    Ok(())
}

fn lex_ident<'a, I: Iterator<Item = State>>(
    input: &'a str,
    tokens: &mut Vec<Token<'a>>,
    chars: &mut Peekable<I>,
    last_i: usize,
) -> Result<(), anyhow::Error> {
    let mut offset = 0;
    while chars.next().is_some() {
        offset += 1;
        if let Some((_, next_c)) = chars.peek() {
            if !next_c.is_alphanumeric() && *next_c != '_' && *next_c != '-' {
                break;
            }
        }
    }
    let ident = &input[last_i..last_i + offset];
    let token = match ident {
        "param" => Token::Param,
        "cell" => Token::Cell,
        x => Token::Ident(x),
    };
    tokens.push(token);
    Ok(())
}

fn lex_comment<'a, I: Iterator<Item = State>>(
    input: &'a str,
    tokens: &mut Vec<Token<'a>>,
    chars: &mut I,
    last_i: usize,
) -> Result<(), anyhow::Error> {
    let mut offset = 0;
    while let Some((_, c)) = chars.next() {
        match c {
            '\n' => {
                break;
            }
            _ => offset += 1,
        }
    }
    let comment_text = &input[last_i + 1..last_i + offset];
    tokens.push(Token::Comment(comment_text));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use Token::*;

    macro_rules! test {
        ($name:ident,$data:literal, [$($expected_tokens:pat,)*]) => {
            #[test]
            fn $name() {
                let mut tokens = lex($data).unwrap().into_iter();
                println!("{:?}", tokens);
                $(
                  let token = tokens.next();

                  if !matches!(token, Some($expected_tokens)) {
                      panic!("expected token {:?} found {:?}", stringify!($expected_tokens), token);
                  }
                )*
                let token = tokens.next();
                assert!(token.is_none());
            }
        };
    }

    test! {
        test_lex_comment,
        r#"
#
# comment 1
   # comment 2
   ### comment
        "#,
        [
            Comment(""),
            Comment(" comment 1"),
            Comment(" comment 2"),
            Comment("## comment"),
        ]
    }

    test! {
        test_lex_param,
        r#"
param a;
param aa;
param a_b: 1;
param a_b_c : "test";
        "#,
        [
            Param, Ident("a"), SemiColon,
            Param, Ident("aa"), SemiColon,
            Param, Ident("a_b"), Colon, NumberLiteral("1"), SemiColon,
            Param, Ident("a_b_c"), Colon, StringLiteral("test"), SemiColon,
        ]
    }

    test! {
        test_lex_cell,
        r#"
cell cpu_cost:
        total_cpu_core * (
          provider == "gcp"
          ? gcp_cpu_core_cost
          : provider == "aws"
          ? aws_cpu_core_cost
          : azure_cpu_core_cost
        )
;
        "#,
        [
            Cell, Ident("cpu_cost"), Colon,
                Ident("total_cpu_core"), Mul, ParOpen,
                  Ident("provider"), Equal, StringLiteral("gcp"),
                  QuestionMark, Ident("gcp_cpu_core_cost"),
                  Colon, Ident("provider"), Equal, StringLiteral("aws"),
                  QuestionMark, Ident("aws_cpu_core_cost"),
                  Colon, Ident("azure_cpu_core_cost"),
                ParClose,
            SemiColon,
        ]
    }

    test! {
        test_lex_string,
        r#"
"a" "bbbb" "\n\nn" "" "t\tt"
        "#,
        [
            StringLiteral("a"),
            StringLiteral("bbbb"),
            StringLiteral("\n\nn"),
            StringLiteral(""),
            StringLiteral("t\tt"),
        ]
    }

    test! {
        test_lex_number,
        r#"
1
0
9
11
32
1.0
13.50
-13.50
- 13.50
+13.50 test
+ 13.50
        "#,
        [
            NumberLiteral("1"),
            NumberLiteral("0"),
            NumberLiteral("9"),
            NumberLiteral("11"),
            NumberLiteral("32"),
            NumberLiteral("1.0"),
            NumberLiteral("13.50"),
            NumberLiteral("-13.50"),
            Sub, NumberLiteral("13.50"),
            NumberLiteral("+13.50"), Ident("test"),
            Add, NumberLiteral("13.50"),
        ]
    }
}
