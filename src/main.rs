use crate::analyser::Analyser;
use std::env;
use std::fs;

mod lexer;
mod analyser;

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
}