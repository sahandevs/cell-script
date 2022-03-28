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
pub type CallStack = Vec<String>;

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

pub fn run_expr(
    expr: &Expr,
    context: &mut ExecutionContext,
    call_stack: &mut CallStack,
) -> Result<f64, anyhow::Error> {
    let result = match expr {
        Expr::Atom(x) => match x {
            Number(x) => Ok(*x),
            Ident(cell_name) => {
                if call_stack.iter().find(|x| *x == cell_name).is_some() {
                    bail!("cyclic dependency found. {:?} -> {}", call_stack, cell_name)
                }
                let cell = context.find_cell(cell_name)?;
                let result = match cell {
                    CellResult::Pending(x) => {
                        call_stack.push(cell_name.clone());
                        run_expr(x, context, call_stack)?
                    }
                    CellResult::Done(x) => *x,
                };
                Ok(result)
            }
        },
        Expr::Add(l, r) => {
            Ok(run_expr(l, context, call_stack)? + run_expr(r, context, call_stack)?)
        }
        Expr::Sub(l, r) => {
            Ok(run_expr(l, context, call_stack)? - run_expr(r, context, call_stack)?)
        }
        Expr::Mul(l, r) => {
            Ok(run_expr(l, context, call_stack)? * run_expr(r, context, call_stack)?)
        }
        Expr::Div(l, r) => {
            Ok(run_expr(l, context, call_stack)? / run_expr(r, context, call_stack)?)
        }
    };
    call_stack.pop();
    result
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
    let mut call_stack = Vec::with_capacity(10);
    call_stack.push(cell_name.to_owned());
    let result = match cell {
        CellResult::Pending(x) => run_expr(x, &mut context, &mut call_stack)?,
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
    fn test_expect_error(code: &str, cell_name: &str) {
        let ast = parser::parse(scanner::scan(code).unwrap()).unwrap();
        if let Ok(x) = run(&ast, cell_name, &HashMap::new()) {
            panic!("expected error but got {}", x);
        }
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
        test_expect_error(
            r#"
        cell a: b;
        cell b: a;
        "#,
            "b",
        );
        test_expect_error(
            r#"
        cell a: b;
        cell b: c;
        cell c: a;
        "#,
            "b",
        );
    }
}
