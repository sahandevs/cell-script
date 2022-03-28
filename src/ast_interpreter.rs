use std::collections::HashMap;

use anyhow::bail;

use crate::parser::{
    Atom::{Ident, Number},
    Expr, Node, AST,
};

/*

cell a:
  1 + 1
;

cell b:
  a * 5
;

*/

pub type Params = HashMap<String, f64>;

#[derive(Debug)]
pub enum CellResult<'a> {
    Pending(&'a Expr),
    Done(f64),
}

#[derive(Default, Debug)]
pub struct ExecutionContext<'a> {
    pub cell_results: HashMap<&'a str, CellResult<'a>>,
}

impl<'a> ExecutionContext<'a> {
    pub fn find_cell(&self, cell_name: &str) -> Result<&CellResult<'a>, anyhow::Error> {
        if let Some(cell) = self.cell_results.get(cell_name) {
            Ok(cell)
        } else {
            bail!("`{}` is not defined", cell_name);
        }
    }
}

pub fn run_expr(expr: &Expr, context: &mut ExecutionContext) -> Result<f64, anyhow::Error> {
    match expr {
        Expr::Atom(x) => match x {
            Number(x) => Ok(*x),
            Ident(cell_name) => {
                let cell = context.find_cell(cell_name)?;
                let result = match cell {
                    CellResult::Pending(x) => run_expr(x, context)?,
                    CellResult::Done(x) => *x,
                };
                Ok(result)
            }
        },
        Expr::Add(l, r) => Ok(run_expr(l, context)? + run_expr(r, context)?),
        Expr::Sub(l, r) => Ok(run_expr(l, context)? - run_expr(r, context)?),
        Expr::Mul(l, r) => Ok(run_expr(l, context)? * run_expr(r, context)?),
        Expr::Div(l, r) => Ok(run_expr(l, context)? / run_expr(r, context)?),
    }
}

pub fn run(code: &AST, cell_name: &str, params: &Params) -> Result<f64, anyhow::Error> {
    let mut context = ExecutionContext::default();
    for node in &code.nodes {
        match node {
            Node::Cell(cell) => {
                context
                    .cell_results
                    .insert(&cell.name, CellResult::Pending(&cell.expr));
            }
            Node::Param(value) => {
                let name = &value.name;
                if let Some(value) = params.get(name) {
                    context.cell_results.insert(name, CellResult::Done(*value));
                } else {
                    bail!("param `{}` not found", name);
                }
            }
        }
    }
    let cell = context.find_cell(cell_name)?;
    let result = match cell {
        CellResult::Pending(x) => run_expr(x, &mut context)?,
        CellResult::Done(x) => *x,
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::scanner;

    #[track_caller]
    fn test(code: &str, cell_name: &str) -> f64 {
        let ast = parser::parse(scanner::scan(code).unwrap()).unwrap();
        run(&ast, cell_name, &HashMap::new()).unwrap()
    }

    #[track_caller]
    fn test_with_param(code: &str, cell_name: &str, params: &Params) -> f64 {
        let ast = parser::parse(scanner::scan(code).unwrap()).unwrap();
        run(&ast, cell_name, params).unwrap()
    }

    #[test]
    fn test_simple() {
        assert_eq!(
            test(
                r#"
            cell a: 1 + 2;
            "#,
                "a"
            ),
            3f64
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            "#,
                "a"
            ),
            6f64
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            cell b: 3 + 2;
            "#,
                "b"
            ),
            5f64
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            cell b: a + 2;
            "#,
                "b"
            ),
            8f64
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            cell b: a + 2;
            cell c: b + b;
            "#,
                "c"
            ),
            16f64
        );
    }

    macro_rules! test_with_param {
        ($code:expr, $cell_name:expr, { $(
            $key:expr => $value: expr,
        )* }) => {
            {
                let mut params = HashMap::new();
                $(
                    params.insert($key.to_owned(), $value);
                )*
                test_with_param($code, $cell_name, &params)
            }
        };
    }

    #[test]
    fn test_param() {
        assert_eq!(
            test_with_param!(
                r#"
            param test;
            cell a: test + 2;
            "#,
                "a",
                {
                    "test" => 5f64,
                }
            ),
            7f64
        );
        assert_eq!(
            test_with_param!(
                r#"
            param test1;
            param test2;
            cell a: test1 + 2;
            cell b: test1 + test2 + a;
            "#,
                "b",
                {
                    "test1" => 2f64,
                    "test2" => 3f64,
                }
            ),
            9f64
        );
    }

    #[test]
    fn test_cyclic() {
        assert_eq!(
            test(
                r#"
            cell a: b;
            cell b: a;
            "#,
                "b"
            ),
            8f64
        );
    }
}
