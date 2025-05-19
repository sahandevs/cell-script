use std::collections::HashMap;

use cranelift::prelude::*;
use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::immediates::Offset32;
use cranelift_codegen::ir::{types::*, Block};
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, UserFuncName};
use cranelift_codegen::verifier::verify_function;
use cranelift_codegen::{settings, write_function};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};

use crate::ir as CellIR;

// copied from https://github.com/bytecodealliance/cranelift-jit-demo/blob/main/src/jit.rs
/// The basic JIT class.
pub struct JIT {
    /// The function builder context, which is reused across multiple
    /// FunctionBuilder instances.
    builder_context: FunctionBuilderContext,

    /// The main Cranelift context, which holds the state for codegen. Cranelift
    /// separates this from `Module` to allow for parallel compilation, with a
    /// context per thread, though this isn't in the simple demo here.
    ctx: codegen::Context,

    /// The data description, which is to data objects what `ctx` is to functions.
    data_description: DataDescription,

    /// The module, with the jit backend, which manages the JIT'd
    /// functions.
    module: JITModule,
}

impl Default for JIT {
    fn default() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        flag_builder.set("opt_level", "speed").unwrap();

        let flags = settings::Flags::new(flag_builder);

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder.finish(flags).unwrap();
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_description: DataDescription::new(),
            module,
        }
    }
}

pub fn jit_compile(ir: &CellIR::IR, param_order: &[&str]) -> String {
    let mut jit = JIT::default();

    // in array
    jit.ctx
        .func
        .signature
        .params
        .push(AbiParam::new(jit.module.target_config().pointer_type()));
    // out array
    jit.ctx
        .func
        .signature
        .params
        .push(AbiParam::new(jit.module.target_config().pointer_type()));

    let mut fn_builder_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut jit.ctx.func, &mut fn_builder_ctx);

        let block0 = builder.create_block();

        /* stack and temporary vars */
        let mut stack_vars: Vec<Variable> = vec![];
        let mut current_stack: Vec<Variable> = vec![];
        macro_rules! stack {
            (pop) => {
                current_stack.pop().expect("stack must not be empty")
            };
            (push) => {{
                let var = if stack_vars.len() <= current_stack.len() {
                    let var = Variable::new(stack_vars.len());
                    builder.declare_var(var, F64);
                    stack_vars.push(var);
                    var
                } else {
                    stack_vars[current_stack.len()].clone()
                };

                current_stack.push(var);
                var
            }};
            (push + $expr:expr) => {{
                let x = current_stack[$expr].clone();
                current_stack.push(x.clone());
                x
            }};
            (push $var:expr) => {{
                let x = $var;
                current_stack.push(x.clone());
                var
            }};
        }
        /* end of stack and temporary vars */

        builder.append_block_params_for_function_params(block0);
        builder.seal_block(block0);
        builder.switch_to_block(block0);

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
            println!(".{} {:?}", i, inst);
            if let Some(block) = blocks.get(&i) {
                println!("..switching to {:?}", block);
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
                    stack!(push + *offset);
                }
                CellIR::Instruction::LoadParam(name) => {
                    let in_val = *builder.block_params(block0).first().unwrap();
                    let out = stack!(push);

                    let offset = param_order.iter().position(|x| x == name).unwrap();

                    let out_val = builder.ins().load(
                        F64,
                        MemFlags::trusted(),
                        in_val,
                        Offset32::new((offset * 0) as _),
                    );
                    builder.def_var(out, out_val);
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

        /*

        the final resulting stack is enough, we can use metadata to determine which offset
        is what. example run:
        v0 = test1
        v1 = test2

        params var [var0, var1] ["test1", "test2"]
        cell var [var2, var3, var4, var5] ["test1", "a", "test2", "b"]

        .0 LoadParam("test1")     [v0]
        v3 = .1 LoadConst(2.0)    [v0, v3]
        v4 = .2 LoadConst(13.0)   [v0, v3, v4]
        v5 = .3 Add               [v0, v5]
        .4 Read(0)                [v0, v5, v0]
        v6 = .5 Add               [v0, v6]
        .6 LoadParam("test2")     [v0, v6, v1]
        .7 Read(1)                [v0, v6, v1, v6]
        .8 Read(2)                [v0, v6, v1, v6, v1]
        v7 = .9 Add               [v0, v6, v1, v7]
        .10 Read(0)               [v0, v6, v1, v7, v0]
        v8 =.11 Add               [v0, v6, v1, v8]
                 */

        // return cell results in order in the last block
        let vals: Vec<_> = stack_vars.iter().map(|var| builder.use_var(*var)).collect();

        let out_val = *builder.block_params(block0).last().unwrap();
        for (i, val) in vals.iter().enumerate() {
            builder.ins().store(
                MemFlags::trusted(),
                *val,
                out_val,
                Offset32::new((i * 8) as _),
            );
        }

        builder.ins().return_(&[]);
        builder.seal_all_blocks();
        builder.finalize();
    }

    // let flags = settings::Flags::new(settings::builder());
    // jit.ctx.
    // let res = verify_function(&jit.ctx.func, &flags);
    // println!("{}", jit.ctx.func.display());
    // if let Err(errors) = res {
    //     panic!("{}", errors);
    // }

    let id = jit
        .module
        .declare_function("run", Linkage::Export, &jit.ctx.func.signature)
        .map_err(|e| e.to_string())
        .unwrap();

    // Define the function to jit. This finishes compilation, although
    // there may be outstanding relocations to perform. Currently, jit
    // cannot finish relocations until all functions to be called are
    // defined. For this toy demo for now, we'll just finalize the
    // function below.
    jit.module
        .define_function(id, &mut jit.ctx)
        .map_err(|e| e.to_string())
        .unwrap();

    let mut buf = String::new();
    write_function(&mut buf, &jit.ctx.func).unwrap();
    println!("{}", buf);
    // Now that compilation is finished, we can clear out the context state.
    jit.module.clear_context(&mut jit.ctx);

    // Finalize the functions which we just defined, which resolves any
    // outstanding relocations (patching in addresses, now that they're
    // available).
    jit.module.finalize_definitions().unwrap();

    // We can now retrieve a pointer to the machine code.
    let code = jit.module.get_finalized_function(id);
    unsafe {
        let code_fn = std::mem::transmute::<_, fn(*mut f64, *mut f64)>(code);
        let mut out = [0f64, 0f64, 0f64, 0f64];
        code_fn([1f64, 2f64].as_mut_ptr(), out.as_mut_ptr());
        println!("run result = {:?}", out);
    }

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
        let code = r#"
        param test1;
        param test2;
        cell a: test1 + 13 + 2;
        cell b: test1 + test2 + a;
        "#;
        let ast = parse(code);
        let ir = code_gen(&ast).unwrap();
        println!("{:?}", ir);

        pretty_assertions::assert_eq!(
            super::jit_compile(&ir, &["test1", "test2"]).trim(),
            r#"
function u0:0(i64, i64) system_v {
block0(v0: i64, v1: i64):
    jump block1

block1:
    v2 = load.f64 notrap aligned v0
    store notrap aligned v2, v1
    v10 = f64const 0x1.e000000000000p3
    v6 = fadd v2, v10  ; v10 = 0x1.e000000000000p3
    store notrap aligned v6, v1+8
    store notrap aligned v2, v1+16
    v8 = fadd v2, v6
    v9 = fadd v2, v8
    store notrap aligned v9, v1+24
    return
}
        "#
            .trim()
        );
    }
}
