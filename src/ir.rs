use std::collections::HashMap;

use anyhow::Ok;
use itertools::Itertools;

use crate::parser::{Atom, Expr, Node, Operator, AST};

pub struct IR<'a> {
    pub instructions: Vec<Instruction<'a>>,
}

pub type InstAddr = usize;

#[derive(Debug)]
pub enum Slot<'a> {
    Accumulator,
    Var(usize),
    CellResult(&'a str),
    Const(f64),
}

#[derive(Debug)]
pub enum Instruction<'a> {
    /// compare A op arg and jump to addr
    JMPCompare {
        op: &'a Operator,
        arg: Slot<'a>,
        addr: InstAddr,
    },
    JMP(InstAddr),
    /// A = A + Slot
    Add(Slot<'a>),
    /// A = A - Slot
    Sub(Slot<'a>),
    /// A = A * Slot
    Mul(Slot<'a>),
    /// A = A % Slot
    Mod(Slot<'a>),
    /// do nothing
    Nop,
    /// A = A / Slot
    Div(Slot<'a>),
    /// A = fn_name()
    /// calling convention:
    /// args will be passed as `Var(arg_start + 0), Var(arg_start + 1), Var(arg_start + 2), ...`
    Call {
        fn_name: &'a str,
        arg_start: usize,
    },
    Mov {
        from: Slot<'a>,
        to: Slot<'a>,
    },
    LoadParam {
        param: &'a str,
        to: Slot<'a>,
    },
}

impl IR<'_> {
    pub fn text(&self) -> String {
        use std::fmt::Write;
        let mut result = String::new();
        for (addr, inst) in self.instructions.iter().enumerate() {
            let _ = writeln!(result, "{}\t:{:?}", addr, inst);
        }
        result
    }
}

/////////////////
/// code gen
/////////////////

pub fn code_gen(ast: &AST) -> Result<IR, anyhow::Error> {
    // find dependency graph

    let order_of_execution = get_order_of_execution(ast)?;

    let mut ir = IR {
        instructions: vec![],
    };

    for item in order_of_execution {
        let node = ast.find_node(item).expect("unreachable");
        match node {
            Node::Param(x) => ir.instructions.push(Instruction::LoadParam {
                param: &x.name,
                to: Slot::CellResult(&x.name),
            }),
            Node::Cell(x) => {
                let mut sp = 0;
                code_gen_expr(&mut sp, &x.expr, &mut ir)?;
                ir.instructions.push(Instruction::Mov {
                    from: Slot::Accumulator,
                    to: Slot::CellResult(&x.name),
                })
            }
        }
    }

    Ok(ir)
}

pub fn code_gen_expr<'a>(
    sp: &mut usize,
    expr: &'a Expr,
    ir: &mut IR<'a>,
) -> Result<(), anyhow::Error> {
    match expr {
        Expr::Atom(x) => match x {
            Atom::Number(x) => ir.instructions.push(Instruction::Mov {
                from: Slot::Const(*x),
                to: Slot::Accumulator,
            }),
            Atom::Ident(x) => ir.instructions.push(Instruction::Mov {
                from: Slot::CellResult(x),
                to: Slot::Accumulator,
            }),
            Atom::Call { name, arguments } => {
                let arg_start = *sp;
                for (i, arg) in arguments.iter().enumerate() {
                    code_gen_expr(sp, arg, ir)?;
                    ir.instructions.push(Instruction::Mov {
                        from: Slot::Accumulator,
                        to: Slot::Var(*sp + i),
                    });
                    *sp += 1;
                }
                ir.instructions.push(Instruction::Call {
                    fn_name: name,
                    arg_start,
                })
            }
        },
        Expr::Add(l, r) => {
            code_gen_expr(sp, l, ir)?;
            // mov Left result in Var(0)
            let target = *sp;
            ir.instructions.push(Instruction::Mov {
                from: Slot::Accumulator,
                to: Slot::Var(target),
            });
            *sp += 1;
            // Right result is in Accumulator
            code_gen_expr(sp, r, ir)?;
            // A = Var(0) `Left` + A `Right`
            ir.instructions.push(Instruction::Add(Slot::Var(target)));
        }
        Expr::Mul(l, r) => {
            code_gen_expr(sp, l, ir)?;
            // mov Left result in Var(0)
            let target = *sp;
            ir.instructions.push(Instruction::Mov {
                from: Slot::Accumulator,
                to: Slot::Var(target),
            });
            *sp += 1;
            // Right result is in Accumulator
            code_gen_expr(sp, r, ir)?;
            // A = Var(0) `Left` * A `Right`
            ir.instructions.push(Instruction::Mul(Slot::Var(target)));
        }
        // order is important so we are calculating right first and
        // moving it in Var(0)
        Expr::Mod(l, r) => {
            code_gen_expr(sp, r, ir)?;
            let target = *sp;
            ir.instructions.push(Instruction::Mov {
                from: Slot::Accumulator,
                to: Slot::Var(target),
            });
            *sp += 1;
            code_gen_expr(sp, l, ir)?;
            ir.instructions.push(Instruction::Mod(Slot::Var(target)));
        }
        Expr::Sub(l, r) => {
            code_gen_expr(sp, r, ir)?;
            let target = *sp;
            ir.instructions.push(Instruction::Mov {
                from: Slot::Accumulator,
                to: Slot::Var(target),
            });
            *sp += 1;
            code_gen_expr(sp, l, ir)?;
            ir.instructions.push(Instruction::Sub(Slot::Var(target)));
        }
        Expr::Div(l, r) => {
            code_gen_expr(sp, r, ir)?;
            let target = *sp;
            ir.instructions.push(Instruction::Mov {
                from: Slot::Accumulator,
                to: Slot::Var(target),
            });
            *sp += 1;
            code_gen_expr(sp, l, ir)?;
            ir.instructions.push(Instruction::Div(Slot::Var(target)));
        }
        Expr::Condition {
            lhs,
            rhs,
            op,
            true_branch,
            false_branch,
        } => {
            code_gen_expr(sp, rhs, ir)?;
            let rhs_val = *sp;
            ir.instructions.push(Instruction::Mov {
                from: Slot::Accumulator,
                to: Slot::Var(rhs_val),
            });
            *sp += 1;
            code_gen_expr(sp, lhs, ir)?;
            // A == left & Var(0) == right
            // placeholder jump instructions
            ir.instructions.push(Instruction::Nop);
            let jmp_cmp_addr = ir.instructions.len() - 1;
            // false branch == don't jump so the layout will be something like this:
            //      JMPCompare
            //      false_branch
            //      false_branch
            //      false_branch
            //      JMP :end
            // a:   true_branch
            //      true_branch
            //      true_branch
            //      true_branch
            // end: NOP
            code_gen_expr(sp, false_branch, ir)?;
            ir.instructions.push(Instruction::Nop);
            let false_branch_jmp_addr = ir.instructions.len() - 1;
            let true_branch_start_addr = ir.instructions.len();
            code_gen_expr(sp, true_branch, ir)?;
            ir.instructions.push(Instruction::Nop);
            let last_nop_instr_addr = ir.instructions.len() - 1;

            ir.instructions[jmp_cmp_addr] = Instruction::JMPCompare {
                op,
                arg: Slot::Var(rhs_val),
                addr: true_branch_start_addr,
            };
            ir.instructions[false_branch_jmp_addr] = Instruction::JMP(last_nop_instr_addr);
        }
    };
    Ok(())
}

/// find order of execution by finding dependency graph
fn get_order_of_execution(ast: &AST) -> Result<Vec<&str>, anyhow::Error> {
    // node -> dependencies
    let mut dep_map = HashMap::new();
    for node in &ast.nodes {
        match node {
            Node::Param(p) => dep_map.insert(p.name.as_str(), Vec::with_capacity(0)),
            Node::Cell(cell) => dep_map.insert(cell.name.as_str(), cell.expr.name_uses()),
        };
    }
    let mut order = Vec::new();

    fn update_order_in_subset<'a>(
        graph: &HashMap<&'a str, Vec<&'a str>>,
        node: (&'a str, &Vec<&'a str>),
        order: &Vec<&'a str>,
    ) -> Vec<&'a str> {
        let (name, deps) = node;
        if order.contains(&name) {
            return vec![];
        }

        let deps: Vec<_> = deps
            .iter()
            .filter(|x| !order.contains(x))
            .map(|x| (*x, graph.get(x).unwrap()))
            .collect();
        let mut result = vec![];
        for node in deps {
            result.extend(update_order_in_subset(graph, node, order));
        }

        if !order.contains(&name) {
            result.push(name);
        }
        result
    }

    for (name, deps) in dep_map.iter().sorted_by_key(|x| &*x.0) {
        order.extend(update_order_in_subset(&dep_map, (name, deps), &order));
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    use crate::scanner;
    fn parse(input: &str) -> AST {
        let tokens = scanner::scan(input).unwrap();
        crate::parser::parse(tokens).unwrap()
    }

    fn compare_generated_code(code: &str, expected: &str) {
        let ast = parse(code);
        let ir = code_gen(&ast).unwrap();
        let text = ir.text().replace('\t', "");
        println!("{}", text);
        let generated: Vec<_> = text.trim().lines().map(str::trim).collect();
        let expected: Vec<_> = expected.trim().lines().map(str::trim).collect();
        assert_eq!(generated, expected);
    }

    #[test]
    fn test_code_gen_for_example1() {
        compare_generated_code(
            include_str!("../examples/example_1.cell"),
            r#"
            0:LoadParam { param: "p1", to: CellResult("p1") }
            1:Mov { from: CellResult("p1"), to: Accumulator }
            2:Mov { from: Accumulator, to: Var(0) }
            3:Mov { from: Const(1.0), to: Accumulator }
            4:Add(Var(0))
            5:Mov { from: Accumulator, to: CellResult("c") }
            6:Mov { from: Const(5.0), to: Accumulator }
            7:Mov { from: Accumulator, to: Var(0) }
            8:Mov { from: CellResult("c"), to: Accumulator }
            9:Add(Var(0))
            10:Mov { from: Accumulator, to: CellResult("a") }
            11:LoadParam { param: "p2", to: CellResult("p2") }
            12:Mov { from: CellResult("p1"), to: Accumulator }
            13:Mov { from: Accumulator, to: Var(0) }
            14:Mov { from: CellResult("p2"), to: Accumulator }
            15:Mov { from: Accumulator, to: Var(1) }
            16:Mov { from: CellResult("p2"), to: Accumulator }
            17:Mov { from: Accumulator, to: Var(2) }
            18:Mov { from: CellResult("a"), to: Accumulator }
            19:Mov { from: Accumulator, to: Var(3) }
            20:Mov { from: CellResult("p2"), to: Accumulator }
            21:Mov { from: Accumulator, to: Var(4) }
            22:Mov { from: Const(1.0), to: Accumulator }
            23:Add(Var(4))
            24:Mov { from: Accumulator, to: Var(5) }
            25:Mov { from: CellResult("p1"), to: Accumulator }
            26:Div(Var(5))
            27:Mul(Var(3))
            28:Add(Var(2))
            29:Mul(Var(1))
            30:Add(Var(0))
            31:Mov { from: Accumulator, to: CellResult("test") }
        "#,
        );
    }

    #[test]
    fn test_code_gen_for_example2() {
        compare_generated_code(
            include_str!("../examples/example_2.cell"),
            r#"
            0:Call { fn_name: "rand", arg_start: 0 }
            1:Mov { from: Accumulator, to: Var(0) }
            2:Mov { from: Const(22.0), to: Accumulator }
            3:Mul(Var(0))
            4:Mov { from: Accumulator, to: Var(1) }
            5:Mov { from: Const(3.0), to: Accumulator }
            6:Add(Var(1))
            7:Mov { from: Accumulator, to: CellResult("F") }
            8:LoadParam { param: "studentnumber", to: CellResult("studentnumber") }
            9:Mov { from: Const(0.0), to: Accumulator }
            10:Mov { from: Accumulator, to: Var(0) }
            11:Mov { from: Const(2.0), to: Accumulator }
            12:Mov { from: Accumulator, to: Var(1) }
            13:Mov { from: CellResult("studentnumber"), to: Accumulator }
            14:Mod(Var(1))
            15:JMPCompare { op: Equals, arg: Var(0), addr: 21 }
            16:Mov { from: Const(3.0), to: Accumulator }
            17:Mov { from: Accumulator, to: Var(2) }
            18:Mov { from: CellResult("studentnumber"), to: Accumulator }
            19:Div(Var(2))
            20:JMP(25)
            21:Mov { from: Const(2.0), to: Accumulator }
            22:Mov { from: Accumulator, to: Var(3) }
            23:Mov { from: CellResult("studentnumber"), to: Accumulator }
            24:Div(Var(3))
            25:Nop
            26:Mov { from: Accumulator, to: CellResult("f") }
            27:Call { fn_name: "rand", arg_start: 0 }
            28:Mov { from: Accumulator, to: Var(0) }
            29:Mov { from: Const(10.0), to: Accumulator }
            30:Mul(Var(0))
            31:Mov { from: Accumulator, to: Var(1) }
            32:Call { fn_name: "int", arg_start: 0 }
            33:Mov { from: Accumulator, to: CellResult("t") }
            34:Call { fn_name: "rand", arg_start: 0 }
            35:Mov { from: Accumulator, to: Var(0) }
            36:Mov { from: Const(4.5), to: Accumulator }
            37:Mul(Var(0))
            38:Mov { from: Accumulator, to: Var(1) }
            39:Mov { from: Const(0.5), to: Accumulator }
            40:Add(Var(1))
            41:Mov { from: Accumulator, to: CellResult("m") }
            42:Mov { from: Const(2.0), to: Accumulator }
            43:Mov { from: Accumulator, to: Var(0) }
            44:Mov { from: CellResult("m"), to: Accumulator }
            45:Mul(Var(0))
            46:Mov { from: Accumulator, to: Var(1) }
            47:Mov { from: CellResult("t"), to: Accumulator }
            48:Mul(Var(1))
            49:Mov { from: Accumulator, to: Var(2) }
            50:Mov { from: CellResult("f"), to: Accumulator }
            51:Div(Var(2))
            52:Mov { from: Accumulator, to: Var(3) }
            53:Mov { from: CellResult("t"), to: Accumulator }
            54:Mov { from: Accumulator, to: Var(4) }
            55:Mov { from: CellResult("f"), to: Accumulator }
            56:Div(Var(4))
            57:Sub(Var(3))
            58:Mov { from: Accumulator, to: CellResult("V") }
        "#,
        );
    }

    #[test]
    fn test_get_order_of_execution() {
        let ast = parse(
            r#"
        param p1;
        param p2;

        cell test:
            p1 + p2 * p2 + a * p1 / (p2 + 1)
        ;

        cell a:
            5 + c
        ;

        cell c: p1 + 1;
        "#,
        );

        let ooe = get_order_of_execution(&ast).unwrap();
        assert_eq!(ooe, vec!["p1", "c", "a", "p2", "test"]);
        let ast = parse(
            r#"
        param studentnumber;
        cell f: 
          if studentnumber % 2 == 0
            ? studentnumber / 2
            : studentnumber / 3;
        
        cell F: ( rand() * 22 ) + 3;
        
        cell t: int( rand() * 10 );
        
        cell m: ( rand() * 4.5 ) + 0.5;
        
        cell V: (f / t ) - ((f/(2*m) * t));  
        "#,
        );

        let ooe = get_order_of_execution(&ast).unwrap();
        assert_eq!(ooe, vec!["F", "studentnumber", "f", "t", "m", "V"]);
    }
}
