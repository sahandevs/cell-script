use anyhow::bail;

use crate::lexer::{self, Token};

#[derive(Debug)]
pub enum Expression {
    Ref(String),
    Add(Box<Expression>, Box<Expression>),
    Mul(Box<Expression>, Box<Expression>),
    Div(Box<Expression>, Box<Expression>),
    Sub(Box<Expression>, Box<Expression>),
    GreaterThan(Box<Expression>, Box<Expression>),
    GreaterThanOrEqual(Box<Expression>, Box<Expression>),
    LessThan(Box<Expression>, Box<Expression>),
    LessThanOrEqual(Box<Expression>, Box<Expression>),
    Equal(Box<Expression>, Box<Expression>),
    If {
        cond: Box<Expression>,
        true_branch: Box<Expression>,
        false_branch: Box<Expression>,
    },
    NumberLiteral(f64),
    StringLiteral(String),
}

pub struct Param {
    pub name: String,
    pub default_value: Option<Expression>,
}

pub struct Cell {
    pub name: String,
    pub body: Expression,
}
pub enum Node {
    Param(Param),
    Cell(Cell),
}

pub struct AST {
    pub nodes: Vec<Node>,
}

pub fn parse_expression(tokens: &[&lexer::Token]) -> Result<(usize, Expression), anyhow::Error> {
    match &tokens[..] {
        [Token::ParOpen, rest @ ..] => {
            let (skip, expression) = parse_expression(rest)?;
            if let Some(Token::ParClose) = tokens.get(skip) {
                Ok((skip + 2, expression))
            } else {
                bail!("closing parenthesis not found for the expression group")
            }
        }
        [left, op, right, ..] => {
            let (_, left) = parse_expression(&[left])?;
            let (_, right) = parse_expression(&[right])?;
            let (left, right) = (Box::new(left), Box::new(right));
            let expr = match op {
                Token::Mul => Expression::Mul(left, right),
                Token::Div => Expression::Div(left, right),
                Token::Sub => Expression::Sub(left, right),
                Token::Add => Expression::Add(left, right),
                _ => bail!("invalid expression {:?}", &tokens),
            };
            Ok((3, expr))
        }
        [Token::NumberLiteral(x), ..] => Ok((1, Expression::NumberLiteral(x.parse()?))),
        [Token::StringLiteral(x), ..] => Ok((1, Expression::StringLiteral(x.to_string()))),
        x => bail!("invalid expression {:?}", x),
    }
}

pub fn parse(tokens: &[lexer::Token]) -> Result<AST, anyhow::Error> {
    let tokens: Vec<_> = tokens
        .iter()
        .filter(|x| !matches!(x, Token::Comment(_)))
        .collect();
    let mut nodes = vec![];
    let mut skip = 0;
    loop {
        match &tokens[skip..] {
            [Token::Param, Token::Ident(name), Token::SemiColon, ..] => {
                nodes.push(Node::Param(Param {
                    name: name.to_string(),
                    default_value: None,
                }));
                skip += 3;
            }
            [Token::Param, Token::Ident(name), Token::Colon, rest @ ..] => {
                let (x, expression) = parse_expression(rest)?;
                nodes.push(Node::Param(Param {
                    name: name.to_string(),
                    default_value: Some(expression),
                }));
                skip += 3 + x;
                if let Some(Token::SemiColon) = tokens.get(skip) {
                    skip += 1;
                } else {
                    bail!("expected semicolon")
                }
            }
            [] => break,
            x => bail!("invalid syntax {:?}", x),
        }
    }

    Ok(AST { nodes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_input(input: &str) -> AST {
        let tokens = lex(input).unwrap();
        let ast = parse(&tokens).unwrap();
        ast
    }

    fn assert_parse_fails(input: &str) {
        let tokens = lex(input).unwrap();
        assert!(parse(&tokens).is_err());
    }

    #[test]
    fn parse_param_with_no_default() {
        let result = parse_input("param hello;");
        assert_eq!(result.nodes.len(), 1);
        let param = match &result.nodes[0] {
            Node::Param(x) => x,
            _ => panic!("expected param"),
        };
        assert_eq!(param.name, "hello");
    }

    #[test]
    fn parse_param_with_default_number() {
        let result = parse_input("param hello: 1;");
        assert_eq!(result.nodes.len(), 1);
        let param = match &result.nodes[0] {
            Node::Param(x) => x,
            _ => panic!("expected param"),
        };
        assert_eq!(param.name, "hello");
        let default = &param.default_value;
        match default {
            Some(Expression::NumberLiteral(x)) if *x == 1.0 => {}
            x => panic!("invalid expression {:?}", x),
        };
    }

    #[test]
    fn parse_param_with_simple_expression() {
        let result = parse_input("param hello: 1 + 5;");
        assert_eq!(result.nodes.len(), 1);
        let param = match &result.nodes[0] {
            Node::Param(x) => x,
            _ => panic!("expected param"),
        };
        assert_eq!(param.name, "hello");
        let default = &param.default_value;
        match default {
            Some(Expression::Add(left, right)) => {
                if let Expression::NumberLiteral(left) = **left {
                    assert_eq!(left, 1.0);
                } else {
                    panic!("expected number")
                }
                if let Expression::NumberLiteral(right) = **right {
                    assert_eq!(right, 5.0);
                } else {
                    panic!("expected number")
                }
            }
            x => panic!("invalid expression {:?}", x),
        };
    }

    #[test]
    fn parse_param_with_default_string() {
        let result = parse_input("param hello: \"test\";");
        assert_eq!(result.nodes.len(), 1);
        let param = match &result.nodes[0] {
            Node::Param(x) => x,
            _ => panic!("expected param"),
        };
        assert_eq!(param.name, "hello");
        let default = &param.default_value;
        match default {
            Some(Expression::StringLiteral(x)) if *x == "test" => {}
            x => panic!("invalid expression {:?}", x),
        };
    }

    #[test]
    fn parse_multi_node_param() {
        let result = parse_input(
            r#"
        param hello: "test";
        param test2: 1;
        param hi: "test2";
        "#,
        );
        assert_eq!(result.nodes.len(), 3);
        let (param1, param2, param3) = match &result.nodes[..] {
            [Node::Param(a), Node::Param(b), Node::Param(c)] => (a, b, c),
            _ => panic!("expected param"),
        };
        assert_eq!(param1.name, "hello");
        let default = &param1.default_value;
        match default {
            Some(Expression::StringLiteral(x)) if *x == "test" => {}
            x => panic!("invalid expression {:?}", x),
        };
        assert_eq!(param2.name, "test2");
        let default = &param2.default_value;
        match default {
            Some(Expression::NumberLiteral(x)) if *x == 1.0 => {}
            x => panic!("invalid expression {:?}", x),
        };
        assert_eq!(param3.name, "hi");
        let default = &param3.default_value;
        match default {
            Some(Expression::StringLiteral(x)) if *x == "test2" => {}
            x => panic!("invalid expression {:?}", x),
        };
    }

    #[test]
    fn parse_param_errors() {
        assert_parse_fails("param hello: 1");
    }
}
