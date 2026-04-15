mod lexer;

fn main() {
    println!("Hello, world!");
    let mut lexer = lexer::Lexer::new(
        r#"
int main() {
    int x = 10;
    int y = 077;
    int z = 0x1A2F;
    double a = 12.5;
    double b = 1e-3;
    double z = 2.31e-43;
    char c = 'k';
    if (x<=y&&z!=0||y>z && z==2) return 0;
    // comment
    char s[20]= "test";
}
"#,
    );
    let tokens = lexer.get_tokens();

    println!("Tokens: {:?}", tokens);
    for token in tokens {
        println!("({:?},{:?})", token.line, token.code);
    }
}
