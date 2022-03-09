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
pub enum Expr {
    Atom(Atom),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
}

#[derive(PartialEq, Debug)]
pub enum Atom {
    Number(f64),
    Ident(String),
}

impl<'a> Token<'a> {
    fn is_operator(&self) -> bool {
        match self {
            Token::Mul | Token::Add | Token::Sub | Token::Div => true,
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
    match token {
        Token::Ident(x) => Ok(Atom::Ident(x.to_string())),
        Token::Number(x) => {
            let number: f64 = x.parse()?;
            Ok(Atom::Number(number))
        }
        x => bail!("[7] unexpected token {:?}", x),
    }
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
