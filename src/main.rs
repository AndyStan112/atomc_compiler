use crate::syntactic::Parser;

mod lexer;
mod syntactic;

fn main() {
    println!("Hello, world!");
    let mut lexer = lexer::Lexer::new(
        r#"
    struct testStruct{
    int a;
    char b;
    };
        double l;
int main() {
    x+ 4;
    x= x+4;
    x-=x+4;
    int x;x = 10;
    int y;y = 077;
    int z;z = 0x1A2F;
    double a;a = 12.5;
    double b;b = 1e-3;
    double z;z = 2.31e-43;
    char c;c = 'k';
    if (x<=y&&z!=0||y>z && z==2) return 0;
    // comment
    char s[20];
    s[1]='c';
}
"#,
    );
    let tokens = lexer.get_tokens();

    println!("Tokens: {:?}", tokens);
    for token in tokens {
        println!("({:?},{:?})", token.line, token.code);
    }

    let mut parser = Parser::new(tokens);

    parser.parse();
}
