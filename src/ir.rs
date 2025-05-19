use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
};

use anyhow::{bail, Ok};
use itertools::Itertools;

use crate::parser::{Atom, Expr, Node, Operator, AST};

pub struct IR<'a> {
    pub instructions: Vec<Instruction<'a>>,
    pub meta: CodeGenMeta<'a>,
}

pub type InstAddr = usize;

/// Stack Pointer offset
pub type SPOffset = usize;

#[derive(Debug)]
pub enum Instruction<'a> {
    /// compare A op st and jump to addr
    JMPIfFalse(InstAddr),
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Equal,
    NotEqual,
    LoadConst(f64),
    JMP(InstAddr),
    Read(SPOffset),
    /// push(pop() + pop())
    Add,
    /// push(pop() - pop())
    Sub,
    /// push(pop() * pop())
    Mul,
    /// push(pop() % pop())
    Mod,
    /// push(pop() / pop())
    Div,
    Nop,
    /// A = fn_name()
    /// calling convention:
    /// args will be passed as `Var(arg_start + 0), Var(arg_start + 1), Var(arg_start + 2), ...`
    Call(&'a str),
    /// push(load())
    LoadParam(&'a str),
}

impl<'a> Instruction<'a> {
    fn fmt(&self, ir: &'a IR<'a>, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JMPIfFalse(arg0) => f.debug_tuple("JMPIfFalse").field(arg0).finish(),
            Self::Greater => write!(f, "Greater"),
            Self::GreaterEqual => write!(f, "GreaterEqual"),
            Self::Less => write!(f, "Less"),
            Self::LessEqual => write!(f, "LessEqual"),
            Self::Equal => write!(f, "Equal"),
            Self::NotEqual => write!(f, "NotEqual"),
            Self::LoadConst(arg0) => f.debug_tuple("LoadConst").field(arg0).finish(),
            Self::JMP(arg0) => f.debug_tuple("JMP").field(arg0).finish(),
            Self::Read(arg0) => {
                // name
                let mut name = "unknown";
                for (key, offset) in &ir.meta.cell_addrs {
                    if *offset == *arg0 {
                        name = key;
                        break;
                    }
                }
                f.debug_tuple("Read").field(&name as _).finish()
            }
            Self::Add => write!(f, "Add"),
            Self::Sub => write!(f, "Sub"),
            Self::Mul => write!(f, "Mul"),
            Self::Mod => write!(f, "Mod"),
            Self::Div => write!(f, "Div"),
            Self::Nop => write!(f, "Nop"),
            Self::Call(arg0) => f.debug_tuple("Call").field(arg0).finish(),
            Self::LoadParam(arg0) => f.debug_tuple("LoadParam").field(arg0).finish(),
        }
    }
}

impl<'a> Debug for IR<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (addr, inst) in self.instructions.iter().enumerate() {
            let _ = write!(f, "{}\t:", addr);
            let _ = inst.fmt(self, f);
            let _ = write!(f, "\n");
        }
        std::fmt::Result::Ok(())
    }
}

impl IR<'_> {
    pub fn text(&self) -> String {
        format!("{:?}", self)
    }
}

/////////////////
/// code gen
/////////////////

#[derive(Debug, Default)]
pub struct CodeGenMeta<'a> {
    pub cell_addrs: indexmap::IndexMap<&'a str, SPOffset>,
    pub st: SPOffset,
}

impl<'a> CodeGenMeta<'a> {
    pub fn find_cell(&self, name: &'a str) -> Result<SPOffset, anyhow::Error> {
        self.cell_addrs
            .get(name)
            .map(|x| *x)
            .ok_or(anyhow::Error::msg("ident not defined"))
    }

    pub fn add_cell(&mut self, name: &'a str, spo: SPOffset) {
        self.cell_addrs.insert(name, spo);
    }

    pub fn add_st(&mut self) -> SPOffset {
        let old = self.st;
        self.st += 1;
        old
    }
}

pub fn code_gen(ast: &AST) -> Result<IR, anyhow::Error> {
    // find dependency graph

    let order_of_execution = get_order_of_execution(ast)?;

    let mut ir = IR {
        instructions: vec![],
        meta: CodeGenMeta::default(),
    };

    for item in order_of_execution {
        let node = ast.find_node(item).expect("unreachable");
        match node {
            Node::Param(x) => {
                ir.instructions.push(Instruction::LoadParam(&x.name));
                let spo = ir.meta.add_st();
                ir.meta.add_cell(&x.name, spo);
            }
            Node::Cell(x) => {
                code_gen_expr(&x.expr, &mut ir)?;
                let spo = ir.meta.add_st();
                ir.meta.add_cell(&x.name, spo);
            }
        }
    }

    Ok(ir)
}

pub fn code_gen_expr<'a>(expr: &'a Expr, ir: &mut IR<'a>) -> Result<(), anyhow::Error> {
    match expr {
        Expr::Atom(Atom::Number(x)) => {
            ir.instructions.push(Instruction::LoadConst(*x));
        }
        Expr::Atom(Atom::Ident(name)) => {
            ir.instructions
                .push(Instruction::Read(ir.meta.find_cell(name)?));
        }
        Expr::Atom(Atom::Call { name, arguments }) => {
            match name.as_str() {
                "rand" => {}
                "int" => {
                    if arguments.len() != 1 {
                        bail!("int expects 1 argument")
                    }
                    code_gen_expr(&arguments[0], ir)?;
                }
                x => {
                    bail!("undefined function {}", x);
                }
            }
            ir.instructions.push(Instruction::Call(name));
        }
        Expr::Add(lhs, rhs) => {
            code_gen_expr(rhs, ir)?;
            code_gen_expr(lhs, ir)?;
            ir.instructions.push(Instruction::Add);
        }
        Expr::Mod(lhs, rhs) => {
            code_gen_expr(rhs, ir)?;
            code_gen_expr(lhs, ir)?;
            ir.instructions.push(Instruction::Mod);
        }
        Expr::Sub(lhs, rhs) => {
            code_gen_expr(rhs, ir)?;
            code_gen_expr(lhs, ir)?;
            ir.instructions.push(Instruction::Sub);
        }
        Expr::Mul(lhs, rhs) => {
            code_gen_expr(rhs, ir)?;
            code_gen_expr(lhs, ir)?;
            ir.instructions.push(Instruction::Mul);
        }
        Expr::Div(lhs, rhs) => {
            code_gen_expr(rhs, ir)?;
            code_gen_expr(lhs, ir)?;
            ir.instructions.push(Instruction::Div);
        }
        Expr::Condition {
            lhs,
            rhs,
            op,
            true_branch,
            false_branch,
        } => {
            code_gen_expr(rhs, ir)?;
            code_gen_expr(lhs, ir)?;
            match op {
                Operator::Equals => ir.instructions.push(Instruction::Equal),
                Operator::Greater => ir.instructions.push(Instruction::Greater),
                Operator::GreaterEqual => ir.instructions.push(Instruction::GreaterEqual),
                Operator::Less => ir.instructions.push(Instruction::Less),
                Operator::LessEqual => ir.instructions.push(Instruction::LessEqual),
            };
            ir.instructions.push(Instruction::JMPIfFalse(0));
            let jmp_if_idx = ir.instructions.len() - 1;
            // true branch
            code_gen_expr(true_branch, ir)?;
            ir.instructions.push(Instruction::JMP(0));
            let true_branch_jmp_idx = ir.instructions.len() - 1;
            // false branch
            code_gen_expr(false_branch, ir)?;
            ir.instructions.push(Instruction::Nop);
            let target_idx = ir.instructions.len() - 1;
            // back patching

            // next instruction of true branch (after the jump) is the false branch
            ir.instructions[jmp_if_idx] = Instruction::JMPIfFalse(true_branch_jmp_idx + 1);
            ir.instructions[true_branch_jmp_idx] = Instruction::JMP(target_idx);
        }
    }
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
            0:LoadParam("p1")
            1:LoadConst(1.0)
            2:Read("p1")
            3:Add
            4:Read("c")
            5:LoadConst(5.0)
            6:Add
            7:LoadParam("p2")
            8:LoadConst(1.0)
            9:Read("p2")
            10:Add
            11:Read("p1")
            12:Div
            13:Read("a")
            14:Mul
            15:Read("p2")
            16:Add
            17:Read("p2")
            18:Mul
            19:Read("p1")
            20:Add
        "#,
        );
    }

    #[test]
    fn test_code_gen_for_example2() {
        compare_generated_code(
            include_str!("../examples/example_2.cell"),
            r#"
            0:LoadConst(3.0)
            1:LoadConst(22.0)
            2:Call("rand")
            3:Mul
            4:Add
            5:LoadParam("studentnumber")
            6:LoadConst(0.0)
            7:LoadConst(2.0)
            8:Read("studentnumber")
            9:Mod
            10:Equal
            11:JMPIfFalse(16)
            12:LoadConst(2.0)
            13:Read("studentnumber")
            14:Div
            15:JMP(19)
            16:LoadConst(3.0)
            17:Read("studentnumber")
            18:Div
            19:Nop
            20:LoadConst(10.0)
            21:Call("rand")
            22:Mul
            23:Call("int")
            24:LoadConst(0.5)
            25:LoadConst(4.5)
            26:Call("rand")
            27:Mul
            28:Add
            29:Read("t")
            30:Read("m")
            31:LoadConst(2.0)
            32:Mul
            33:Mul
            34:Read("f")
            35:Div
            36:Read("t")
            37:Read("f")
            38:Div
            39:Sub
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
