use std::iter::Peekable;

use anyhow::bail;

#[derive(Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Param,           // param
    Cell,            // cell
    Ident(&'a str),  //
    If,              // if
    QMark,           // ?
    SemiColon,       // ;
    Colon,           // :
    Mul,             // *
    Add,             // +
    Sub,             // -
    Div,             // /
    Number(&'a str), // 1, 1.0, -1
    ParOpen,         // (
    ParClose,        // )
    Comma,           // ,
    Greater,         // >
    GreaterEqual,    // >=
    Less,            // <
    LessEqual,       // <=
    Equal,           // ==
}

pub fn scan<'a>(input: &'a str) -> Result<Vec<Token<'a>>, anyhow::Error> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().enumerate().peekable();

    while let Some((i, c)) = chars.next() {
        match c {
            '#' => {
                'inner: while let Some((_, c)) = chars.next() {
                    if c == '\n' {
                        break 'inner;
                    }
                }
            }
            ',' => tokens.push(Token::Comma),
            '?' => tokens.push(Token::QMark),
            ';' => tokens.push(Token::SemiColon),
            ':' => tokens.push(Token::Colon),
            '+' => tokens.push(Token::Add),
            '*' => tokens.push(Token::Mul),
            '>' if matches!(chars.peek(), Some((_, '='))) => {
                chars.next();
                tokens.push(Token::GreaterEqual);
            }
            '>' => tokens.push(Token::Greater),
            '<' if matches!(chars.peek(), Some((_, '='))) => {
                chars.next();
                tokens.push(Token::LessEqual)
            }
            '<' => tokens.push(Token::Less),
            '=' if matches!(chars.peek(), Some((_, '='))) => {
                chars.next();
                tokens.push(Token::Equal);
            }
            '-' => {
                if let Some((_, next_c)) = chars.peek() {
                    if next_c.is_numeric() {
                        tokens.push(scan_number(input, i, &mut chars)?);
                    } else {
                        tokens.push(Token::Sub);
                    }
                } else {
                    tokens.push(Token::Sub);
                }
            }
            '/' => tokens.push(Token::Div),
            '(' => tokens.push(Token::ParOpen),
            ')' => tokens.push(Token::ParClose),
            x if x.is_whitespace() => { /* skip */ }
            x if x.is_numeric() => {
                tokens.push(scan_number(input, i, &mut chars)?);
            }
            x if x.is_ascii_alphabetic() => {
                tokens.push(scan_ident(input, i, &mut chars)?);
            }
            x => {
                bail!("unexpected character `{}`", x)
            }
        }
    }

    Ok(tokens)
}

fn scan_number<'a, T: Iterator<Item = (usize, char)>>(
    input: &'a str,
    start_char_idx: usize,
    chars: &mut Peekable<T>,
) -> Result<Token<'a>, anyhow::Error> {
    let mut offset = 0;
    let mut number = String::new();
    number.push_str(&input[start_char_idx..start_char_idx]);

    while let Some((_, c)) = chars.peek() {
        let c = *c;
        number.push(c);
        if c == '.' {
            offset += 1;
            chars.next();
        } else {
            match number.parse::<f64>() {
                Ok(_) => {
                    offset += 1;
                    chars.next();
                }
                Err(_) => {
                    break;
                }
            }
        }
    }

    let number = &input[start_char_idx..=start_char_idx + offset];
    Ok(Token::Number(number))
}

fn scan_ident<'a, T: Iterator<Item = (usize, char)>>(
    input: &'a str,
    start_char_idx: usize,
    chars: &mut Peekable<T>,
) -> Result<Token<'a>, anyhow::Error> {
    let mut offset = 0;
    while let Some((_, c)) = chars.peek() {
        if c.is_alphanumeric() {
            offset += 1;
            chars.next();
        } else {
            break;
        }
    }

    let ident = &input[start_char_idx..=start_char_idx + offset];
    let token = match ident {
        "param" => Token::Param,
        "cell" => Token::Cell,
        "if" => Token::If,
        x => Token::Ident(x),
    };
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Token::*;

    #[test]
    fn test_param() {
        assert_eq!(
            scan("param abc;").unwrap(),
            vec![Param, Ident("abc"), SemiColon,]
        );

        assert_eq!(
            scan("param   a; # test\n\nparam abc   ;").unwrap(),
            vec![Param, Ident("a"), SemiColon, Param, Ident("abc"), SemiColon,]
        );
    }

    #[test]
    fn test_cell() {
        assert_eq!(
            scan(
                r#"
            cell total:
               1 + 1.0
            ;
            cell test:
                if math(1, 2) >= 123
                  ? 2
                  : 3;
            "#
            )
            .unwrap(),
            vec![
                Cell,
                Ident("total"),
                Colon,
                Number("1"),
                Add,
                Number("1.0"),
                SemiColon,
                Cell,
                Ident("test"),
                Colon,
                If,
                Ident("math"),
                ParOpen,
                Number("1"),
                Comma,
                Number("2"),
                ParClose,
                GreaterEqual,
                Number("123"),
                QMark,
                Number("2"),
                Colon,
                Number("3"),
                SemiColon,
            ]
        );

        assert_eq!(
            scan(
                r#"
            cell total:
               1 + 1.0 / 11.01
            ;
            "#
            )
            .unwrap(),
            vec![
                Cell,
                Ident("total"),
                Colon,
                Number("1"),
                Add,
                Number("1.0"),
                Div,
                Number("11.01"),
                SemiColon,
            ]
        );

        assert_eq!(
            scan(
                r#"
            cell total:
               ( 1 + 1.0 )  / 11.01 * ( (10 + 8 ) - 14 + cost )
            ;
            "#
            )
            .unwrap(),
            vec![
                Cell,
                Ident("total"),
                Colon,
                ParOpen,
                Number("1"),
                Add,
                Number("1.0"),
                ParClose,
                Div,
                Number("11.01"),
                Mul,
                ParOpen,
                ParOpen,
                Number("10"),
                Add,
                Number("8"),
                ParClose,
                Sub,
                Number("14"),
                Add,
                Ident("cost"),
                ParClose,
                SemiColon,
            ]
        );
    }

    #[test]
    fn test_number() {
        assert_eq!(scan("123456").unwrap(), vec![Number("123456"),]);
        assert_eq!(scan("12.156").unwrap(), vec![Number("12.156"),]);
        assert_eq!(scan("-123").unwrap(), vec![Number("-123"),]);
        assert_eq!(scan("-123.123").unwrap(), vec![Number("-123.123"),]);
        assert_eq!(scan("- 123").unwrap(), vec![Sub, Number("123"),]);
        assert_eq!(scan("- abc").unwrap(), vec![Sub, Ident("abc"),]);
        // FIXME: assert_eq!(scan("-abc").unwrap(), vec![Sub, Ident("abc"),]);
    }
}
