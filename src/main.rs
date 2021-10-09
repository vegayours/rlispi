use std::collections::LinkedList;
use std::ops::Fn;
use std::rc::Rc;
use std::{collections::HashMap, io::Read};

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

        let mut add_value = |value: Value, state: &mut LinkedList<Value>| {
            println!("Add value: {:?} {:?} {:?}", &value, state, &result);
            match state.back_mut() {
                Some(Value::List(elements)) => {
                    elements.push_back(value);
                }
                None => {
                    result.push(value);
                }
                Some(_) => unreachable!(),
            }
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
    bindings: HashMap<String, Value>,
    local: HashMap<String, Value>,
}

#[derive(Default)]
struct ContextChange {
    bindings: HashMap<String, Option<Value>>,
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
    fn new() -> Context {
        let mut bindings: HashMap<String, Value> = HashMap::new();
        bindings.insert("nil".into(), Value::Nil);
        bindings.insert("true".into(), Value::Bool(true));
        bindings.insert("false".into(), Value::Bool(false));
        bindings.insert(
            "=".into(),
            Value::Function(Function {
                name: "=".into(),
                fun: Rc::new(|ctx, mut args| {
                    if args.is_empty() {
                        return Err("Function '=' called without arguments".into());
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
            "+".into(),
            Value::Function(Function {
                name: "+".into(),
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
            "def".into(),
            Value::Function(Function {
                name: "def".into(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 2 {
                        return Err(format!("Invalid arguments for def: {:?}", args));
                    }
                    match args.pop_front().unwrap() {
                        Value::Symbol(name) => {
                            let value = eval(ctx, args.pop_front().unwrap())?;
                            ctx.bindings.insert(name.clone(), value);
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
            "list".into(),
            Value::Function(Function {
                name: "list".into(),
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
            "first".into(),
            Value::Function(Function {
                name: "first".into(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 1 {
                        return Err("Function 'first' requires 1 argument".into());
                    }
                    if let Value::List(mut elements) = eval(ctx, args.pop_front().unwrap())? {
                        match elements.pop_front() {
                            Some(elem) => Ok(elem),
                            None => Err("Function 'first' requires non-empty list".into()),
                        }
                    } else {
                        Err("Only list is supported for 'first' function".into())
                    }
                }),
            }),
        );
        bindings.insert(
            "rest".into(),
            Value::Function(Function {
                name: "rest".into(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 1 {
                        return Err("Function 'rest' requires 1 argument".into());
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
            "cons".into(),
            Value::Function(Function {
                name: "cons".into(),
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
            "empty?".into(),
            Value::Function(Function {
                name: "empty?".into(),
                fun: Rc::new(|ctx, mut args| {
                    if args.len() != 1 {
                        return Err("Function 'empty' requires 1 argument".into());
                    }
                    if let Value::List(elements) = eval(ctx, args.pop_front().unwrap())? {
                        Ok(Value::Bool(elements.is_empty()))
                    } else {
                        Err("Only list is supported for 'empty' function".into())
                    }
                }),
            }),
        );
        bindings.insert(
            "if".into(),
            Value::Function(Function {
                name: "if".into(),
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
                        return Err("Function 'if' requires 2 or 3 arguments".into());
                    }
                }),
            }),
        );
        bindings.insert(
            "fn".into(),
            Value::Function(Function {
                name: "fn".into(),
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
                        let local_context = ctx.local.clone();
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
                            let mut inner_ctx = Context {
                                bindings: global_ctx.bindings.clone(),
                                local: local_context.clone(),
                            };
                            for (name, bound_node) in bindings.iter().zip(args) {
                                let bound_value = eval(&mut inner_ctx, bound_node)?;
                                inner_ctx.local.insert(name.clone(), bound_value);
                            }
                            let result = eval(&mut inner_ctx, body.clone())?;
                            Ok(result)
                        };
                        Ok(Value::Function(Function {
                            name: "anonymous".into(),
                            fun: Rc::new(f),
                        }))
                    } else {
                        Err("'fn' has form (fn (arg1 arg2 ...) body)".into())
                    }
                }),
            }),
        );
        Context {
            bindings,
            ..Default::default()
        }
    }
    fn apply_context_change(&mut self, context_change: ContextChange) -> ContextChange {
        let mut restore_context_change = ContextChange {
            bindings: HashMap::new(),
        };
        for (k, v) in context_change.bindings {
            if let Some(new_value) = v {
                if let Some(prev_value) = self.local.remove(&k) {
                    restore_context_change
                        .bindings
                        .insert(k.clone(), Some(prev_value));
                } else {
                    restore_context_change.bindings.insert(k.clone(), None);
                }
                self.local.insert(k, new_value);
            } else {
                self.local.remove(&k);
            }
        }
        restore_context_change
    }
}

fn eval(ctx: &mut Context, value: Value) -> Result<Value, String> {
    println!("Eval value: {:?}. Local ctx: {:?}", value, ctx.local);
    match value {
        Value::Symbol(name) => {
            if let Some(val) = ctx.resolve(&name) {
                println!("{} -> {:?}", name, val);
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
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();
    println!("Src: {}", src.trim_end());

    let mut parser = Parser::new();

    let values: Vec<Value> = parser.parse_next(&src).unwrap();

    let mut context = Context::new();

    for elem in values {
        println!("Eval {:?}", elem);
        let result = eval(&mut context, elem).unwrap();
        println!("Result {:?}", result);
    }
}
