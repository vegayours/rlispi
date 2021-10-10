use std::collections::LinkedList;
use std::fs::File;
use std::io::Write;
use std::ops::Fn;
use std::rc::Rc;
use std::{collections::HashMap, io::Read};
use uuid::Uuid;

#[derive(Clone)]
struct Function {
    name: String,
    fun: Rc<dyn Fn(&mut Context, LinkedList<Value>) -> Result<Value, String>>,
}

impl std::fmt::Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Function").field(&self.name).finish()
    }
}

impl std::cmp::PartialEq for Function {
    fn eq(&self, other: &Function) -> bool {
        self.name == other.name
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Bool(bool),
    Nil,
    Integer(i64),
    List(LinkedList<Value>),
    Function(Function),
    Symbol(String),
    String(String),
}

struct Parser {
    state: LinkedList<Value>,
}

impl Parser {
    fn new() -> Parser {
        Parser {
            state: LinkedList::new(),
        }
    }
    fn parse_next(&mut self, src: &str) -> Result<Vec<Value>, String> {
        let mut result: Vec<Value> = Vec::new();

        let mut src = src;

        let mut add_value = |value: Value, state: &mut LinkedList<Value>| match state.back_mut() {
            Some(Value::List(elements)) => {
                elements.push_back(value);
            }
            None => {
                result.push(value);
            }
            Some(_) => unreachable!(),
        };

        loop {
            src = src.trim_start();
            if src.is_empty() {
                break;
            }

            if src.starts_with(";") {
                let end_pos = src.find('\n').unwrap_or(src.len());
                src = &src[end_pos..];
            } else if src.starts_with('(') {
                self.state.push_back(Value::List(LinkedList::new()));
                src = &src[1..];
            } else if src.starts_with(')') {
                match self.state.pop_back() {
                    Some(list_value @ Value::List(..)) => {
                        add_value(list_value, &mut self.state);
                        src = &src[1..];
                    }
                    _ => {
                        return Err(String::from("Unmatched closing parenthesis"));
                    }
                }
            } else if src.starts_with('"') {
                // TODO: Implement escaped characters handling and multi-line strings.
                src = &src[1..];
                if let Some(end_pos) = src.find('"') {
                    add_value(
                        Value::String(String::from(&src[..end_pos])),
                        &mut self.state,
                    );
                    src = &src[end_pos + 1..];
                } else {
                    return Err(format!("Unterminated string: {}", src));
                }
            } else {
                let end_pos = src
                    .find(|c: char| c.is_whitespace() || c == ')')
                    .unwrap_or(src.len());
                let token = &src[..end_pos];
                src = &src[end_pos..];
                if let Ok(i64_value) = str::parse::<i64>(token) {
                    add_value(Value::Integer(i64_value), &mut self.state);
                } else if is_symbol(token) {
                    add_value(Value::Symbol(String::from(token)), &mut self.state);
                } else {
                    return Err(format!("Unsupported token '{}'", token));
                }
            }
        }
        Ok(result)
    }
    fn finish(self) -> Result<(), String> {
        if self.state.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Syntax error, partially parsed state: {:?}",
                self.state
            ))
        }
    }
}

fn is_symbol(token: &str) -> bool {
    match token {
        "+" | "-" | "*" | "/" | "=" | ">" | "<" => true,
        _ => {
            token.starts_with(|x: char| x.is_alphabetic())
                && token
                    .chars()
                    .skip(1)
                    .all(|x: char| x.is_alphanumeric() || x == '?' || x == '/' || x == '_')
        }
    }
}

#[derive(Default, Clone, Debug)]
struct Context {
    bindings: Rc<HashMap<String, Value>>,
    local: HashMap<String, Value>,
}

impl Context {
    fn resolve(&self, key: &str) -> Option<Value> {
        if let Some(local_value) = self.local.get(key) {
            Some(local_value.clone())
        } else if let Some(global_value) = self.bindings.get(key) {
            Some(global_value.clone())
        } else {
            None
        }
    }
    fn import(ctx: &mut Context, args: LinkedList<Value>) -> Result<Value, String> {
        if args.len() != 1 {
            return Err(format!("Import form expects 1 path argument"));
        }
        if let Some(Value::String(path)) = args.front() {
            let mut src = String::new();
            let _size = File::open(path)
                .map(|mut f| f.read_to_string(&mut src))
                .map_err(|e| format!("Can't read file {}, error: {}", path, e))?;
            let mut file_parser = Parser::new();
            for value in file_parser.parse_next(&src)? {
                eval(ctx, value)?;
            }
            file_parser.finish()?;
            Ok(Value::Nil)
        } else {
            Err(format!(
                "Expected string as argument to 'import', got: {:?}",
                args.front()
            ))
        }
    }
    fn new() -> Context {
        let mut bindings: HashMap<String, Value> = HashMap::new();
        bindings.insert("nil".to_string(), Value::Nil);
        bindings.insert("true".to_string(), Value::Bool(true));
        bindings.insert("false".to_string(), Value::Bool(false));
        bindings.insert(
            "=".to_string(),
            Value::Function(Function {
                name: "=".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if args.is_empty() {
                        return Err("Function '=' called without arguments".to_string());
                    }
                    let value = eval(ctx, args.pop_front().unwrap())?;
                    for other in args {
                        if value != eval(ctx, other)? {
                            return Ok(Value::Bool(false));
                        }
                    }
                    Ok(Value::Bool(true))
                }),
            }),
        );
        bindings.insert(
            "+".to_string(),
            Value::Function(Function {
                name: "+".to_string(),
                fun: Rc::new(|ctx, args| {
                    let mut result: i64 = 0;
                    for arg in args {
                        match eval(ctx, arg)? {
                            Value::Integer(value) => {
                                result += value;
                            }
                            other => {
                                return Err(format!("Calling function '+' with arg: {:?}", other));
                            }
                        }
                    }
                    Ok(Value::Integer(result))
                }),
            }),
        );
        bindings.insert(
            "def".to_string(),
            Value::Function(Function {
                name: "def".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 2 {
                        return Err(format!("Invalid arguments for def: {:?}", args));
                    }
                    match args.pop_front().unwrap() {
                        Value::Symbol(name) => {
                            let value = eval(ctx, args.pop_front().unwrap())?;
                            Rc::get_mut(&mut ctx.bindings).unwrap().insert(name.clone(), value);
                            Ok(Value::Nil)
                        }
                        other => Err(format!(
                            "'def' first argument must by symbol, got: {:?}",
                            other
                        )),
                    }
                }),
            }),
        );
        bindings.insert(
            "list".to_string(),
            Value::Function(Function {
                name: "list".to_string(),
                fun: Rc::new(|ctx, args| {
                    let mut list_values: LinkedList<Value> = LinkedList::new();
                    for arg in args {
                        list_values.push_back(eval(ctx, arg)?);
                    }
                    Ok(Value::List(list_values))
                }),
            }),
        );
        bindings.insert(
            "first".to_string(),
            Value::Function(Function {
                name: "first".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 1 {
                        return Err("Function 'first' requires 1 argument".to_string());
                    }
                    if let Value::List(mut elements) = eval(ctx, args.pop_front().unwrap())? {
                        match elements.pop_front() {
                            Some(elem) => Ok(elem),
                            None => Err("Function 'first' requires non-empty list".to_string()),
                        }
                    } else {
                        Err("Only list is supported for 'first' function".to_string())
                    }
                }),
            }),
        );
        bindings.insert(
            "rest".to_string(),
            Value::Function(Function {
                name: "rest".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 1 {
                        return Err("Function 'rest' requires 1 argument".to_string());
                    }
                    let mut list = eval(ctx, args.pop_front().unwrap())?;
                    if let Value::List(elements) = &mut list {
                        if elements.pop_front().is_none() {
                            return Err(String::from("Function 'rest' requires non-empty list"));
                        }
                    }
                    Ok(list)
                }),
            }),
        );
        bindings.insert(
            "cons".to_string(),
            Value::Function(Function {
                name: "cons".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 2 {
                        return Err(String::from("Function 'cons' requires 2 arguments"));
                    }
                    let (head, mut tail) = (
                        eval(ctx, args.pop_front().unwrap())?,
                        eval(ctx, args.pop_front().unwrap())?,
                    );
                    if let Value::List(elements) = &mut tail {
                        elements.push_front(head);
                    } else {
                        return Err(String::from(
                            "Only list is supported for 'cons' function 2nd argument",
                        ));
                    }
                    Ok(tail)
                }),
            }),
        );
        bindings.insert(
            "empty?".to_string(),
            Value::Function(Function {
                name: "empty?".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 1 {
                        return Err("Function 'empty' requires 1 argument".to_string());
                    }
                    if let Value::List(elements) = eval(ctx, args.pop_front().unwrap())? {
                        Ok(Value::Bool(elements.is_empty()))
                    } else {
                        Err("Only list is supported for 'empty' function".to_string())
                    }
                }),
            }),
        );
        bindings.insert(
            "if".to_string(),
            Value::Function(Function {
                name: "if".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if let (Some(condition), Some(true_branch), false_branch, None) = (
                        args.pop_front(),
                        args.pop_front(),
                        args.pop_front(),
                        args.pop_front(),
                    ) {
                        match eval(ctx, condition)? {
                            Value::Bool(false) | Value::Nil => {
                                false_branch.map_or(Ok(Value::Nil), |node| eval(ctx, node))
                            }
                            _ => eval(ctx, true_branch),
                        }
                    } else {
                        return Err("Function 'if' requires 2 or 3 arguments".to_string());
                    }
                }),
            }),
        );
        bindings.insert(
            "fn".to_string(),
            Value::Function(Function {
                name: "fn".to_string(),
                fun: Rc::new(|ctx, mut args| {
                    if let (Some(Value::List(arg_bindings)), Some(body), None) =
                        (args.pop_front(), args.pop_front(), args.pop_front())
                    {
                        let mut bindings: Vec<String> = Vec::new();
                        for arg_binding in arg_bindings {
                            if let Value::Symbol(name) = arg_binding {
                                bindings.push(name.clone());
                            } else {
                                return Err(format!(
                                    "Function arguments must be symbols, got {:?}.",
                                    arg_binding
                                ));
                            }
                        }
                        let local_copy = ctx.local.clone();
                        let f = move |global_ctx: &mut Context,
                                      args: LinkedList<Value>|
                              -> Result<Value, String> {
                            if bindings.len() != args.len() {
                                return Err(format!(
                                    "Wrong number of arguments, expected {}, got {}",
                                    bindings.len(),
                                    args.len()
                                ));
                            }
                            let mut local_ctx = Context {
                                bindings: global_ctx.bindings.clone(),
                                local: local_copy.clone(),
                            };
                            for (name, bound_node) in bindings.iter().zip(args) {
                                let bound_value = eval(global_ctx, bound_node)?;
                                local_ctx.local.insert(name.clone(), bound_value);
                            }
                            let result = eval(&mut local_ctx, body.clone())?;
                            Ok(result)
                        };
                        Ok(Value::Function(Function {
                            name: Uuid::new_v4().to_string(),
                            fun: Rc::new(f),
                        }))
                    } else {
                        Err("'fn' has form (fn (arg1 arg2 ...) body)".to_string())
                    }
                }),
            }),
        );
        bindings.insert(
            "import".to_string(),
            Value::Function(Function {
                name: "import".to_string(),
                fun: Rc::new(Context::import),
            }),
        );
        Context {
            bindings: Rc::new(bindings),
            ..Default::default()
        }
    }
}

fn eval(ctx: &mut Context, value: Value) -> Result<Value, String> {
    match value {
        Value::Symbol(name) => {
            if let Some(val) = ctx.resolve(&name) {
                Ok(val)
            } else {
                Err(format!("Can't resolve symbol '{}'", name))
            }
        }
        Value::List(mut elements) => {
            if let Some(head) = elements.pop_front() {
                match eval(ctx, head)? {
                    Value::Function(Function { fun, .. }) => fun(ctx, elements),
                    other => Err(format!("Value {:?} is not a function", other)),
                }
            } else {
                return Err(String::from("Can't evaluate empty list"));
            }
        }
        value => Ok(value),
    }
}

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
