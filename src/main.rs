use std::env;
use std::fs::File;
use std::io::{Read, Write};

mod eval;
mod parser;
mod value;

use eval::{eval, Context};
use parser::Parser;

fn interactive() {
    let mut parser = Parser::new();
    let mut context = Context::new();

    let mut src = String::new();
    loop {
        print!("(lispi)=> ");
        std::io::stdout().flush().unwrap();
        if std::io::stdin().read_line(&mut src).unwrap() == 0 {
            println!("");
            parser
                .finish()
                .expect("Partially parsed state on Parser::finish");
            break;
        } else {
            let elems = match parser.parse_next(&src) {
                Ok(elems) => elems,
                Err(err) => {
                    println!("Parse error: {}", err);
                    parser = Parser::new();
                    continue;
                }
            };
            for elem in elems {
                match eval(&mut context, elem) {
                    Ok(result) => {
                        println!("{:?}", result)
                    }
                    Err(err) => println!("Evaluation error: {}", err),
                };
            }
        }
        src.clear();
    }
}

fn eval_file(path: &str) {
    let mut src = String::new();
    let _size = File::open(path)
        .map(|mut f| f.read_to_string(&mut src))
        .map_err(|e| format!("Can't read file {}, error: {}", path, e))
        .unwrap();

    let mut parser = Parser::new();
    let mut context = Context::new();
    for value in parser.parse_next(&src).unwrap() {
        eval(&mut context, value).unwrap();
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        interactive();
    } else {
        eval_file(args.iter().skip(1).next().unwrap());
    }
}
