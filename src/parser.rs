use crate::lexer;

pub enum Atom {
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
}

type Expression = Atom;

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
    nodes: Vec<Node>,
}

pub fn parse(tokens: &[lexer::Token]) -> AST {
    AST { nodes: vec![] }
}

#[cfg(test)]
mod tests {}
