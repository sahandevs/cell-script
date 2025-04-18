pub mod ast_interpreter;
pub mod cli;
pub mod lsp;
pub mod ir;
pub mod vm;
pub mod parser;
pub mod scanner;
pub mod cranelift;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("[Error] {}", e);
    }
}
