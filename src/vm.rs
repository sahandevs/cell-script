use anyhow::Ok;
use rand::random;

use crate::{
    ast_interpreter::Params,
    ir::{self, SPOffset},
};

#[derive(Debug)]
struct Stack<const SIZE: usize> {
    pub data: [f64; SIZE],
    pub st: SPOffset,
}

impl<const SIZE: usize> Stack<SIZE> {
    fn default() -> Self {
        Self {
            data: [0f64; SIZE],
            st: 0,
        }
    }

    #[inline(always)]
    fn push(&mut self, data: f64) {
        self.data[self.st] = data;
        self.st += 1;
    }

    #[inline(always)]
    fn pop(&mut self) -> f64 {
        self.st -= 1;
        let data = self.data[self.st];
        data
    }

    #[inline(always)]
    fn peak(&self, spo: SPOffset) -> f64 {
        self.data[spo]
    }
}

pub fn run<'a>(ir: &'a ir::IR, params: &'a Params) -> Result<Vec<(String, f64)>, anyhow::Error> {
    let mut ip = 0;

    // TODO: we should find a proper stack size
    // from code gen phase
    let mut stack = Stack::<64>::default();

    while let Some(inst) = ir.instructions.get(ip) {
        use ir::Instruction::*;

        match inst {
            JMPIfFalse(x) => {
                if stack.pop() == 0f64 {
                    ip = *x;
                    continue;
                }
            }
            Greater => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(if lhs > rhs { 1f64 } else { 0f64 });
            }
            GreaterEqual => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(if lhs >= rhs { 1f64 } else { 0f64 });
            }
            Less => {
                let rhs = stack.pop();
                let lhs = stack.pop();
                stack.push(if lhs < rhs { 1f64 } else { 0f64 });
            }
            LessEqual => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(if lhs <= rhs { 1f64 } else { 0f64 });
            }
            Equal => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(if lhs == rhs { 1f64 } else { 0f64 });
            }
            NotEqual => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(if lhs != rhs { 1f64 } else { 0f64 });
            }
            LoadConst(x) => stack.push(*x),
            JMP(x) => {
                ip = *x;
                continue;
            }
            Read(x) => stack.push(stack.peak(*x)),
            Add => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(lhs + rhs);
            }
            Sub => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(lhs - rhs);
            }
            Mul => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(lhs * rhs);
            }
            Mod => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(lhs % rhs);
            }
            Div => {
                let lhs = stack.pop();
                let rhs = stack.pop();
                stack.push(lhs / rhs);
            }
            Nop => {}
            LoadParam(x) => {
                let v = params
                    .get(*x)
                    .ok_or(anyhow::Error::msg("param not found"))?;
                stack.push(*v);
            }
            Call(name) => match *name {
                "int" => {
                    let v = stack.pop();
                    stack.push(v.floor())
                }
                "rand" => stack.push(random()),
                _ => {}
            },
        }

        ip += 1;
    }

    let mut result = Params::new();
    for (name, spo) in &ir.meta.cell_addrs {
        result.insert(name.to_string(), stack.peak(*spo));
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
        assert_eq!(
            test(
                r#"
            cell a: 1 - 2;
            cell b: 2 - 1;
            "#,
            ),
            vec![-1f64, 1f64]
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
