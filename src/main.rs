pub mod ast_interpreter;
pub mod cli;
pub mod parser;
pub mod scanner;

fn main() {
    cli::run().unwrap();
}
