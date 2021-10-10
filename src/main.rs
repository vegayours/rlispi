use std::io::Write;

mod eval;
mod parser;
mod value;

use eval::{eval, Context};
use parser::Parser;

fn main() {
    let mut parser = Parser::new();
    let mut context = Context::new();

    let mut src = String::new();
    loop {
        print!("(lispi)=> ");
        std::io::stdout().flush().unwrap();
        if std::io::stdin().read_line(&mut src).unwrap() == 0 {
            println!("");
            parser.finish().unwrap();
            break;
        } else {
            for elem in parser.parse_next(&src).unwrap() {
                let result = eval(&mut context, elem).unwrap();
                println!("{:?}", result)
            }
        }
        src.clear();
    }
}
