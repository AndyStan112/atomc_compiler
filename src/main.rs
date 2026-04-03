mod lexer;

fn main() {
    println!("Hello, world!");
    let mut lexer = lexer::Lexer::new("test\ntest2 same_line2");
    let tokens = lexer.get_tokens();

    println!("Tokens: {:?}", tokens);
    for token in tokens {
        println!("({:?},{:?})", token.line, token.code);
    }
}
