use std::collections::HashMap;

use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::{types::*, Block};
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, Signature, UserFuncName};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::verifier::verify_function;
use cranelift_codegen::{settings, write_function};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};

use crate::ir as CellIR;

pub fn jit_compile(ir: &CellIR::IR) -> String {
    let mut sig = Signature::new(CallConv::Fast);

    let param_to_sig_idx: Vec<_> = ir
        .instructions
        .iter()
        .filter_map(|x| {
            if let CellIR::Instruction::LoadParam(param) = x {
                Some(*param)
            } else {
                None
            }
        })
        .collect();
    for _ in param_to_sig_idx.iter() {
        sig.params.push(AbiParam::new(F64));
    }

    let cell_ret_sig_idx: Vec<_> = ir.meta.cell_addrs.iter().map(|(k, _)| *k).collect();
    for _ in cell_ret_sig_idx.iter() {
        sig.returns.push(AbiParam::new(F64));
    }

    let mut fn_builder_ctx = FunctionBuilderContext::new();
    let mut func = Function::with_name_signature(UserFuncName::user(0, 0), sig);
    {
        let mut builder = FunctionBuilder::new(&mut func, &mut fn_builder_ctx);

        let block0 = builder.create_block();
        // function parameters
        let mut param_vars = vec![];
        for (i, _) in param_to_sig_idx.iter().enumerate() {
            let var = Variable::new(i);
            builder.declare_var(var, F64);
            param_vars.push(var);
        }

        // storage for cell results:
        let mut cell_vars = vec![];
        for (i, _) in cell_ret_sig_idx.iter().enumerate() {
            let var = Variable::new(param_vars.len() + i);
            builder.declare_var(var, F64);
            cell_vars.push(var);
        }

        /* stack and temporary vars */
        let mut stack_vars: Vec<Variable> = vec![];
        let mut current_stack: Vec<Variable> = vec![];
        macro_rules! stack {
            (pop) => {
                current_stack.pop().expect("stack must not be empty")
            };
            (push) => {{
                let var = if stack_vars.len() <= current_stack.len() {
                    let var = Variable::new(param_vars.len() + cell_vars.len() + stack_vars.len());
                    builder.declare_var(var, F64);
                    stack_vars.push(var);
                    var
                } else {
                    stack_vars[current_stack.len()].clone()
                };

                current_stack.push(var);
                var
            }};
        }
        /* end of stack and temporary vars */

        builder.append_block_params_for_function_params(block0);
        builder.seal_block(block0);
        builder.switch_to_block(block0);
        // assign function parameters from block
        {
            let block_params: Vec<_> = builder.block_params(block0).iter().cloned().collect();
            for (block, par) in block_params.iter().zip(param_vars.iter()) {
                builder.def_var(par.clone(), *block);
            }
        }

        let mut blocks: HashMap<usize, Block> = HashMap::new();
        let entry_block = builder.create_block();
        builder.ins().jump(entry_block, &[]);
        builder.seal_block(entry_block);
        blocks.insert(0, entry_block);
        /* segment instruction based on jump instruction targets. key of blocks hash map is the
          the start of a block.
        */
        for (i, inst) in ir.instructions.iter().enumerate() {
            match inst {
                CellIR::Instruction::JMPIfFalse(target) => {
                    blocks
                        .entry(*target)
                        .or_insert_with(|| builder.create_block());
                    // if compare fails, next instruction is the start of the new block
                    blocks
                        .entry(i + 1)
                        .or_insert_with(|| builder.create_block());
                }
                CellIR::Instruction::JMP(addr) => {
                    blocks
                        .entry(*addr)
                        .or_insert_with(|| builder.create_block());
                }
                _ => {}
            }
        }

        for (i, inst) in ir.instructions.iter().enumerate() {
            if let Some(block) = blocks.get(&i) {
                builder.switch_to_block(*block);
            }
            match inst {
                CellIR::Instruction::Call(_) => todo!(),
                CellIR::Instruction::JMPIfFalse(target) => {
                    let a = builder.use_var(stack!(pop));
                    let target_block = *blocks.get(target).unwrap();
                    let next_block = *blocks.get(&(i + 1)).unwrap();
                    builder.ins().brif(a, target_block, &[], next_block, &[]);
                }
                CellIR::Instruction::JMP(target) => {
                    builder.ins().jump(*blocks.get(target).unwrap(), &[]);
                }
                CellIR::Instruction::Nop => {}

                CellIR::Instruction::LoadConst(val) => {
                    let out = stack!(push);
                    let tmp = builder.ins().f64const(*val);
                    builder.def_var(out, tmp);
                }
                CellIR::Instruction::Read(offset) => {
                    // convert offset to the name then name to variable
                    let name = ir
                        .meta
                        .cell_addrs
                        .iter()
                        .find(|(_, v)| *v == offset)
                        .map(|x| *x.0)
                        .unwrap();
                    let idx = cell_ret_sig_idx.iter().position(|x| *x == name).unwrap();

                    let out = stack!(push);
                    let val = builder.use_var(cell_vars[idx]);
                    builder.def_var(out, val);
                }
                CellIR::Instruction::LoadParam(name) => {
                    let out = stack!(push);
                    let idx = param_to_sig_idx.iter().position(|x| x == name).unwrap();
                    let val = builder.use_var(param_vars[idx]);
                    builder.def_var(out, val);
                }

                // TODO: factor out duplicate code
                // TODO: compare inst return int, we should cast them into float and
                // convert it back to int in conditional jump
                CellIR::Instruction::Greater => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));

                    let result = builder.ins().fcmp(FloatCC::GreaterThan, a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::GreaterEqual => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));

                    let result = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::Less => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));

                    let result = builder.ins().fcmp(FloatCC::LessThan, a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::LessEqual => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));

                    let result = builder.ins().fcmp(FloatCC::LessThanOrEqual, a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::Equal => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));

                    let result = builder.ins().fcmp(FloatCC::Equal, a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::NotEqual => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));

                    let result = builder.ins().fcmp(FloatCC::NotEqual, a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::Add => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));
                    let result = builder.ins().fadd(a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::Sub => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));
                    let result = builder.ins().fsub(a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::Mul => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));
                    let result = builder.ins().fmul(a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::Div => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));
                    let result = builder.ins().fdiv(a, b);
                    let out = stack!(push);
                    builder.def_var(out, result);
                }
                CellIR::Instruction::Mod => {
                    let a = builder.use_var(stack!(pop));
                    let b = builder.use_var(stack!(pop));

                    // TODO: isn't there any direct instruction for float rem op?
                    // r = a - b * floor(a/b)
                    let div = builder.ins().fdiv(a, b);
                    let floor_div = builder.ins().floor(div);
                    let prod = builder.ins().fmul(b, floor_div);
                    let result = builder.ins().fsub(a, prod);

                    let out = stack!(push);
                    builder.def_var(out, result);
                }
            }
        }

        // return cell results in order in the last block
        let vals: Vec<_> = cell_vars.iter().map(|var| builder.use_var(*var)).collect();
        builder.ins().return_(&vals);

        builder.seal_all_blocks();
        builder.finalize();
    }

    let flags = settings::Flags::new(settings::builder());

    let res = verify_function(&func, &flags);
    println!("{}", func.display());
    if let Err(errors) = res {
        panic!("{}", errors);
    }

    let mut buf = String::new();
    write_function(&mut buf, &func).unwrap();
    // TODO: https://github.com/bytecodealliance/cranelift-jit-demo/blob/main/src/bin/toy.rs
    buf
}

#[cfg(test)]
mod tests {
    use crate::{ir::code_gen, parser::AST, scanner};

    fn parse(input: &str) -> AST {
        let tokens = scanner::scan(input).unwrap();
        crate::parser::parse(tokens).unwrap()
    }

    #[test]
    pub fn test_example_1() {
        let code = include_str!("../examples/example_1.cell");
        let ast = parse(code);
        let ir = code_gen(&ast).unwrap();

        assert_eq!(
            super::jit_compile(&ir).trim(),
            r#"
function u0:0(f64, f64) -> f64, f64, f64, f64, f64 fast {
block0(v0: f64, v1: f64):
    v22 = f64const 0.0
    v21 -> v22
    v16 = f64const 0.0
    v15 -> v16
    v12 = f64const 0.0
    v11 -> v12
    v7 = f64const 0.0
    v6 -> v7
    v4 = f64const 0.0
    v3 -> v4
    jump block1

block1:
    v2 = f64const 0x1.0000000000000p0
    v5 = fadd.f64 v4, v2  ; v4 = 0.0, v2 = 0x1.0000000000000p0
    v8 = f64const 0x1.4000000000000p2
    v9 = fadd v8, v7  ; v8 = 0x1.4000000000000p2, v7 = 0.0
    v10 = f64const 0x1.0000000000000p0
    v13 = fadd.f64 v12, v10  ; v12 = 0.0, v10 = 0x1.0000000000000p0
    v14 = fdiv.f64 v3, v13  ; v3 = 0.0
    v17 = fmul.f64 v16, v14  ; v16 = 0.0
    v18 = fadd.f64 v11, v17  ; v11 = 0.0
    v19 = fmul.f64 v11, v18  ; v11 = 0.0
    v20 = fadd.f64 v3, v19  ; v3 = 0.0
    return v6, v11, v15, v3, v22  ; v6 = 0.0, v11 = 0.0, v15 = 0.0, v3 = 0.0, v22 = 0.0
}
        "#
            .trim()
        );
    }
}
