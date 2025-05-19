pub mod ast_interpreter;
pub mod cli;
pub mod cranelift;
pub mod ir;
pub mod parser;
pub mod scanner;
pub mod vm;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("[Error] {}", e);
    }
}
