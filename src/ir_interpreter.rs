use anyhow::{bail, Ok};
use rand::Rng;

use crate::{
    ast_interpreter::Params,
    ir::{self, Slot},
    parser::Operator,
};

impl Operator {
    fn compare(&self, left: f64, right: f64) -> bool {
        match self {
            Operator::Equals => left == right,
            Operator::Greater => left > right,
            Operator::GreaterEqual => left >= right,
            Operator::Less => left < right,
            Operator::LessEqual => left <= right,
        }
    }
}

pub fn run(ir: &ir::IR, params: &Params) -> Result<Vec<(String, f64)>, anyhow::Error> {
    let mut ip = 0;

    let mut a = 0f64;
    let mut stack = [0f64; 10];

    let mut result = Params::new();

    macro_rules! read {
        ($slot:expr) => {{
            match &$slot {
                Slot::Accumulator => a,
                Slot::Var(a) => stack[*a],
                Slot::CellResult(name) => *result.get(*name).unwrap_or(&0f64),
                Slot::Const(x) => *x,
            }
        }};
    }

    macro_rules! get_slot {
        ($slot:ident) => {{
            match $slot {
                Slot::Accumulator => &mut a,
                Slot::Var(a) => &mut stack[*a],
                Slot::CellResult(name) => {
                    if let Some(x) = result.get_mut(*name) {
                        x
                    } else {
                        result.entry(name.to_string()).or_insert(0f64)
                    }
                }
                Slot::Const(_) => bail!("cannot get mutable reference to a const"),
            }
        }};
    }

    while let Some(inst) = ir.instructions.get(ip) {
        use ir::Instruction::*;

        match inst {
            JMPCompare { op, arg, addr } => {
                if op.compare(a, read!(arg)) {
                    ip = *addr;
                }
            }
            JMP(addr) => ip = *addr,
            Add(x) => a += read!(x),
            Sub(x) => a -= read!(x),
            Mul(x) => a *= read!(x),
            Mod(x) => a %= read!(x),
            Div(x) => a /= read!(x),
            Nop => {}
            Mov { from, to } => {
                let from = read!(from);
                let to = get_slot!(to);
                *to = from;
            }
            LoadParam { param, to } => {
                if let Some(param) = params.get(*param) {
                    let to = get_slot!(to);
                    *to = *param;
                } else {
                    bail!("param {} not found", param);
                }
            }
            Call { fn_name, arg_start } => {
                let x = match *fn_name {
                    "rand" => {
                        let mut rng = rand::thread_rng();
                        rng.gen()
                    }
                    "int" => {
                        let arg0 = read!(Slot::Var(*arg_start));
                        arg0.round()
                    }
                    x => bail!("undefined function {}", x),
                };
                a = x;
            }
        }

        ip += 1;
    }

    Ok(result.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use itertools::Itertools;

    use crate::{ir::code_gen, parser, scanner};

    use super::*;

    #[track_caller]
    fn test(code: &str) -> Vec<f64> {
        let ast = parser::parse(scanner::scan(code).unwrap()).unwrap();
        let ir = code_gen(&ast).unwrap();
        println!("{}", ir.text());
        run(&ir, &HashMap::new())
            .unwrap()
            .into_iter()
            .sorted_by_key(|x| x.0.to_string())
            .map(|(_, x)| x)
            .collect()
    }

    #[track_caller]
    fn test_with_param(code: &str, params: &Params) -> Vec<f64> {
        let ast = parser::parse(scanner::scan(code).unwrap()).unwrap();
        let ir = code_gen(&ast).unwrap();
        run(&ir, params)
            .unwrap()
            .into_iter()
            .sorted_by_key(|x| x.0.to_string())
            .map(|(_, x)| x)
            .collect()
    }
    #[test]
    fn test_cond() {
        assert_eq!(
            test(
                r#"
            cell a: if 1 + 2 > 4 ? 10 : 20;
            "#,
            ),
            vec![20f64]
        );
    }

    #[test]
    fn test_simple() {
        assert_eq!(
            test(
                r#"
            cell a: 1 + 2;
            "#,
            ),
            vec![3f64]
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            "#,
            ),
            vec![6f64]
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            cell b: 3 + 2;
            "#,
            ),
            vec![6.0f64, 5f64]
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            cell b: a + 2;
            "#,
            ),
            vec![6.0f64, 8f64]
        );
        assert_eq!(
            test(
                r#"
            cell a: 3 * 2;
            cell b: a + 2;
            cell c: b + b;
            "#,
            ),
            vec![6.0f64, 8.0f64, 16f64]
        );
    }

    macro_rules! test_with_param {
        ($code:expr, { $(
            $key:expr => $value: expr,
        )* }) => {
            {
                let mut params = HashMap::new();
                $(
                    params.insert($key.to_owned(), $value);
                )*
                test_with_param($code, &params)
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
                {
                    "test" => 5f64,
                }
            ),
            vec![7f64, 5f64]
        );
        assert_eq!(
            test_with_param!(
                r#"
            param test1;
            param test2;
            cell a: test1 + 2;
            cell b: test1 + test2 + a;
            "#,
                {
                    "test1" => 2f64,
                    "test2" => 3f64,
                }
            ),
            vec![4f64, 9f64, 2f64, 3f64]
        );
    }
}
