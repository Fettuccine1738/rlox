use crate::scanner::Scanner;
use crate::scanner::Kind;
use crate::scanner::Token;

pub fn compile(source: String) {
    let mut scanner = Scanner::new(&source);
    let mut line= std::u32::MAX;
    loop {
        let token: Token = scanner.scan_token();
        if token.line != line {
            println!("{:4}", token.line);
        } else {
            println!("    | ");
        }
        println!("{:?} '{}'", token.kind, token.lexeme);
        if matches!(token.kind, Kind::EOF) {
            break;
        }
    }
}
