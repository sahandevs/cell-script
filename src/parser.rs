use std::iter::Peekable;

use anyhow::bail;

use crate::scanner::Token;

#[derive(PartialEq, Debug, Default)]
pub struct AST {
    pub nodes: Vec<Node>,
}

#[derive(PartialEq, Debug)]
pub enum Node {
    Param(Param),
    Cell(Cell),
}

#[derive(PartialEq, Debug)]
pub struct Param {
    pub name: String,
}

#[derive(PartialEq, Debug)]
pub struct Cell {
    pub name: String,
    pub expr: Expr,
}

#[derive(PartialEq, Debug)]
pub enum Operator {
    Equals,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(PartialEq, Debug)]
pub enum Expr {
    Atom(Atom),
    Add(Box<Expr>, Box<Expr>),
    Mod(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Condition {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        op: Operator,
        true_branch: Box<Expr>,
        false_branch: Box<Expr>,
    },
}

#[derive(PartialEq, Debug)]
pub enum Atom {
    Number(f64),
    Ident(String),
    Call { name: String, arguments: Vec<Expr> },
}

impl<'a> Token<'a> {
    fn is_operator(&self) -> bool {
        match self {
            Token::Mul | Token::Add | Token::Sub | Token::Div | Token::Mod => true,
            _ => false,
        }
    }
}

fn parse_atom<'a, T: Iterator<Item = Token<'a>>>(
    tokens: &mut Peekable<T>,
) -> Result<Atom, anyhow::Error> {
    let token = tokens
        .next()
        .ok_or_else(|| anyhow::Error::msg("[8] expected a token"))?;
    let next_token = tokens.peek();
    match token {
        Token::Ident(x) if matches!(next_token, Some(Token::ParOpen)) => {
            let fn_name = x;
            // skip para
            tokens.next();
            let mut args = vec![];
            while let Ok(expr) = parse_expr(tokens) {
                args.push(expr);
                let next_token = tokens.peek();
                match next_token {
                    Some(Token::ParClose) => {
                        tokens.next();
                        break;
                    }
                    Some(Token::Comma) => {
                        tokens.next();
                        continue;
                    }
                    x => bail!("invalid token {:?}", x),
                }
            }
            Ok(Atom::Call {
                name: fn_name.to_string(),
                arguments: args,
            })
        }
        Token::Ident(x) => Ok(Atom::Ident(x.to_string())),
        Token::Number(x) => {
            let number: f64 = x.parse()?;
            Ok(Atom::Number(number))
        }
        x => bail!("[7] unexpected token {:?}", x),
    }
}

fn parse_cond<'a, T: Iterator<Item = Token<'a>>>(
    tokens: &mut Peekable<T>,
) -> Result<Expr, anyhow::Error> {
    // skip if
    tokens.next();
    // expr
    let lhs = Box::new(parse_expr(tokens)?);
    // op
    let op = match tokens.next() {
        Some(Token::Greater) => Operator::Greater,
        Some(Token::GreaterEqual) => Operator::GreaterEqual,
        Some(Token::Less) => Operator::Less,
        Some(Token::LessEqual) => Operator::LessEqual,
        Some(Token::Equal) => Operator::Equals,
        x => bail!("unexpected token {:?}", x),
    };
    // expr
    let rhs = Box::new(parse_expr(tokens)?);
    let token = tokens.next();
    if !matches!(token, Some(Token::QMark)) {
        bail!("expected ? found {:?}", token);
    }
    let true_branch = Box::new(parse_expr(tokens)?);
    let token = tokens.next();
    if !matches!(token, Some(Token::Colon)) {
        bail!("expected : found {:?}", token);
    }
    let false_branch = Box::new(parse_expr(tokens)?);
    return Ok(Expr::Condition {
        lhs,
        rhs,
        op,
        true_branch,
        false_branch,
    });
}

fn parse_expr<'a, T: Iterator<Item = Token<'a>>>(
    tokens: &mut Peekable<T>,
) -> Result<Expr, anyhow::Error> {
    let first = tokens
        .peek()
        .ok_or_else(|| anyhow::Error::msg("[6] expected a token"))?;

    let lhs_expr = {
        match first {
            Token::ParOpen => {
                tokens.next();
                let expr = parse_expr(tokens)?;
                match tokens.next() {
                    Some(Token::ParClose) => expr,
                    x => bail!("[5] unexpected token {:?}", x),
                }
            }
            Token::If => {
                let cond = parse_cond(tokens)?;
                cond
            }
            _ => {
                let atom = parse_atom(tokens)?;
                Expr::Atom(atom)
            }
        }
    };

    if let Some(next) = tokens.peek() {
        if next.is_operator() {
            // SAFETY: we already checked with `peek`
            let next = tokens.next().unwrap();
            let rhs_expr = parse_expr(tokens)?;
            Ok(match next {
                Token::Mul => Expr::Mul(Box::new(lhs_expr), Box::new(rhs_expr)),
                Token::Add => Expr::Add(Box::new(lhs_expr), Box::new(rhs_expr)),
                Token::Sub => Expr::Sub(Box::new(lhs_expr), Box::new(rhs_expr)),
                Token::Div => Expr::Div(Box::new(lhs_expr), Box::new(rhs_expr)),
                Token::Mod => Expr::Mod(Box::new(lhs_expr), Box::new(rhs_expr)),
                _ => bail!("unreachable!"),
            })
        } else {
            Ok(lhs_expr)
        }
    } else {
        Ok(lhs_expr)
    }
}

fn parse_cell<'a, T: Iterator<Item = Token<'a>>>(
    tokens: &mut Peekable<T>,
) -> Result<Cell, anyhow::Error> {
    let name = match (tokens.next(), tokens.next()) {
        (Some(Token::Ident(name)), Some(Token::Colon)) => name,
        x => bail!("[4] unexpected token: {:?}", x),
    };
    let expr = parse_expr(tokens)?;
    match tokens.next() {
        Some(Token::SemiColon) => Ok(Cell {
            name: name.to_string(),
            expr,
        }),
        x => bail!("[3] unexpected token: {:?}", x),
    }
}

fn parse_param<'a, T: Iterator<Item = Token<'a>>>(
    tokens: &mut Peekable<T>,
) -> Result<Param, anyhow::Error> {
    match (tokens.next(), tokens.next()) {
        (Some(Token::Ident(name)), Some(Token::SemiColon)) => Ok(Param {
            name: name.to_string(),
        }),
        x => bail!("[2] unexpected token: {:?}", x),
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<AST, anyhow::Error> {
    let mut ast = AST::default();
    let mut tokens = tokens.into_iter().peekable();

    while let Some(token) = tokens.next() {
        match token {
            Token::Param => {
                ast.nodes.push(Node::Param(parse_param(&mut tokens)?));
            }
            Token::Cell => {
                ast.nodes.push(Node::Cell(parse_cell(&mut tokens)?));
            }
            x => bail!("[1] unexpected token {:?}, expected 'param' or 'cell'", x),
        }
    }

    Ok(ast)
}

#[cfg(test)]
mod tests {
    use crate::scanner;
    fn parse(input: &str) -> String {
        let tokens = scanner::scan(input).unwrap();
        format!("{:?}", super::parse(tokens).unwrap())
    }

    #[test]
    fn test_param() {
        assert_eq!(
            parse("param test;"),
            "AST { nodes: [Param(Param { name: \"test\" })] }"
        );
        assert_eq!(
            parse("param test; param test2;"),
            "AST { nodes: [Param(Param { name: \"test\" }), Param(Param { name: \"test2\" })] }"
        );
    }

    #[test]
    fn test_func() {
        assert_eq!(
            parse("cell test: random();"),
            "AST { nodes: [Cell(Cell { name: \"test\", expr: Atom(Call { name: \"random\", arguments: [] }) })] }"
        );
        assert_eq!(
            parse("cell test: random(1);"),
            "AST { nodes: [Cell(Cell { name: \"test\", expr: Atom(Call { name: \"random\", arguments: [Atom(Number(1.0))] }) })] }"
        );
        assert_eq!(
            parse("cell test: random(1, 2, 3) + 1;"),
            "AST { nodes: [Cell(Cell { name: \"test\", expr: Add(Atom(Call { name: \"random\", arguments: [Atom(Number(1.0)), Atom(Number(2.0)), Atom(Number(3.0))] }), Atom(Number(1.0))) })] }"
        );
    }

    #[test]
    fn test_cell() {
        assert_eq!(
            parse(r#"cell test2: 1;"#),
            "AST { nodes: [Cell(Cell { name: \"test2\", expr: Atom(Number(1.0)) })] }"
        );
        assert_eq!(
            parse(
                r#"
        cell test: 1;
        cell test2: 1 + 2;
        "#
            ),
            "AST { nodes: [Cell(Cell { name: \"test\", expr: Atom(Number(1.0)) }), Cell(Cell { name: \"test2\", expr: Add(Atom(Number(1.0)), Atom(Number(2.0))) })] }"
        );
        assert_eq!(parse(r#"cell test2: (1 + 2) + 3;"#), "AST { nodes: [Cell(Cell { name: \"test2\", expr: Add(Add(Atom(Number(1.0)), Atom(Number(2.0))), Atom(Number(3.0))) })] }");
        assert_eq!(parse(r#"cell test2: (1 / abc) + 3;"#), "AST { nodes: [Cell(Cell { name: \"test2\", expr: Add(Div(Atom(Number(1.0)), Atom(Ident(\"abc\"))), Atom(Number(3.0))) })] }");
        assert_eq!(parse(r#"cell test2: (1 + abc) - 3;"#), "AST { nodes: [Cell(Cell { name: \"test2\", expr: Sub(Add(Atom(Number(1.0)), Atom(Ident(\"abc\"))), Atom(Number(3.0))) })] }");
        assert_eq!(parse(r#"cell test2: (1 * abc) - 3;"#), "AST { nodes: [Cell(Cell { name: \"test2\", expr: Sub(Mul(Atom(Number(1.0)), Atom(Ident(\"abc\"))), Atom(Number(3.0))) })] }");
        assert_eq!(parse(r#"cell test2: (-1 * (abc)) - 3;"#), "AST { nodes: [Cell(Cell { name: \"test2\", expr: Sub(Mul(Atom(Number(-1.0)), Atom(Ident(\"abc\"))), Atom(Number(3.0))) })] }");
    }
}
