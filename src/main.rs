pub mod ast_interpreter;
pub mod cli;
pub mod ir;
pub mod vm;
pub mod parser;
pub mod scanner;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("[Error] {}", e);
    }
}
