use crate::analyser::Analyser;
use std::env;
use std::fs;

mod lexer;
mod analyser;
mod vm;
mod layout;
mod compiler;
mod bytecode;

fn main() {
    let filename = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/main.atomc".to_string());

    let source = fs::read_to_string(&filename)
        .unwrap_or_else(|err| {
            eprintln!("Could not read file `{}`: {}", filename, err);
            std::process::exit(1);
        });

    let mut lexer = lexer::Lexer::new(&source);

    let tokens = lexer.get_tokens();

    let mut analyser = Analyser::new(tokens);

    analyser.parse();
    analyser.print_symbols();

    let mut vm = vm::mv_test().unwrap_or_else(|err| {
        eprintln!("Could not build VM test: {}", err);
        std::process::exit(1);
    });

    vm.trace = true;
    vm.run().unwrap_or_else(|err| {
        eprintln!("VM error: {}", err);
        std::process::exit(1);
    });

    print!("{}", vm.output());
}
