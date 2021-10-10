use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;
use uuid::Uuid;
use im_lists::list::List;

use crate::parser::Parser;
use crate::value::{Function, FunctionType, Value};

#[derive(Default, Clone, Debug)]
pub struct Context {
    bindings: Rc<HashMap<String, Value>>,
    local: HashMap<String, Value>,
}

struct OpsEnv;

impl OpsEnv {
    fn add(ctx: &mut Context, args: List<Value>) -> Result<Value, String> {
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
    }
    fn eq(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
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
    }

    fn bind(ctx: &mut Context) {
        ctx.bind_fn("+", &OpsEnv::add);
        ctx.bind_fn("=", &OpsEnv::eq);
    }
}

struct CoreEnv;

impl CoreEnv {
    fn def(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
        if args.len() != 2 {
            return Err(format!("Invalid arguments for def: {:?}", args));
        }
        match args.pop_front().unwrap() {
            Value::Symbol(name) => {
                let value = eval(ctx, args.pop_front().unwrap())?;
                Rc::get_mut(&mut ctx.bindings)
                    .unwrap()
                    .insert(name.clone(), value);
                Ok(Value::Nil)
            }
            other => Err(format!(
                "'def' first argument must by symbol, got: {:?}",
                other
            )),
        }
    }
    fn if_fn(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
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
    }
    fn lambda_fn(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
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
                          args: List<Value>|
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

                // Looping allows us to implement tail call optimisation.
                // By convention we use 'recur' to indicate recursive tail call.
                // TODO: Implement error reporting when using 'recur' in non-tail call position.
                let result = loop {
                    let result = eval(&mut local_ctx, body.clone())?;
                    match result {
                        Value::List(mut elements) => match elements.first() {
                            Some(Value::Symbol(name)) if name == "recur" => {
                                elements.pop_front();
                                if elements.len() != bindings.len() {
                                    return Err(format!("Wrong number of arguments passed to 'recur'. Expected {}, got {}",
                                                       bindings.len(), elements.len()));
                                }
                                let mut arg_values = Vec::with_capacity(bindings.len());
                                for value in elements {
                                    let bound_value = eval(&mut local_ctx, value)?;
                                    arg_values.push(bound_value);
                                }
                                for (name, bound_value) in
                                    bindings.iter().zip(arg_values.into_iter())
                                {
                                    local_ctx.local.insert(name.clone(), bound_value);
                                }
                            }
                            _ => {
                                break Value::List(elements);
                            }
                        },
                        _ => {
                            break result;
                        }
                    };
                };
                Ok(result)
            };
            Ok(Value::Function(Function {
                name: Uuid::new_v4().to_string(),
                fun: Rc::new(f),
            }))
        } else {
            Err("'fn' has form (fn (arg1 arg2 ...) body)".to_string())
        }
    }
    fn import(ctx: &mut Context, args: List<Value>) -> Result<Value, String> {
        if args.len() != 1 {
            return Err(format!("Import form expects 1 path argument"));
        }
        if let Some(Value::String(path)) = args.first() {
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
                args.first()
            ))
        }
    }

    fn bind(ctx: &mut Context) {
        ctx.bind_fn("def", &CoreEnv::def);
        ctx.bind_fn("if", &CoreEnv::if_fn);
        ctx.bind_fn("fn", &CoreEnv::lambda_fn);
        ctx.bind_fn("import", &CoreEnv::import);
    }
}

struct ListEnv;

impl ListEnv {
    fn list(ctx: &mut Context, args: List<Value>) -> Result<Value, String> {
        let mut list_values: List<Value> = List::new();
        for arg in args {
            list_values.push_back(eval(ctx, arg)?);
        }
        Ok(Value::List(list_values))
    }
    fn first(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
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
    }
    fn rest(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
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
    }
    fn cons(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
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
    }
    fn empty(ctx: &mut Context, mut args: List<Value>) -> Result<Value, String> {
        if args.len() != 1 {
            return Err("Function 'empty' requires 1 argument".to_string());
        }
        if let Value::List(elements) = eval(ctx, args.pop_front().unwrap())? {
            Ok(Value::Bool(elements.is_empty()))
        } else {
            Err("Only list is supported for 'empty' function".to_string())
        }
    }

    fn bind(ctx: &mut Context) {
        ctx.bind_fn("list", &ListEnv::list);
        ctx.bind_fn("first", &ListEnv::first);
        ctx.bind_fn("rest", &ListEnv::rest);
        ctx.bind_fn("cons", &ListEnv::cons);
        ctx.bind_fn("empty?", &ListEnv::empty);
    }
}

impl Context {
    pub fn new() -> Context {
        let mut ctx = Context {
            bindings: Rc::new(HashMap::new()),
            local: HashMap::new(),
        };
        ctx.bind_value("nil", Value::Nil);
        ctx.bind_value("true", Value::Bool(true));
        ctx.bind_value("false", Value::Bool(false));
        CoreEnv::bind(&mut ctx);
        OpsEnv::bind(&mut ctx);
        ListEnv::bind(&mut ctx);
        ctx
    }
    pub fn resolve(&self, key: &str) -> Option<Value> {
        if let Some(local_value) = self.local.get(key) {
            Some(local_value.clone())
        } else if let Some(global_value) = self.bindings.get(key) {
            Some(global_value.clone())
        } else {
            None
        }
    }
    fn bind_value(&mut self, name: &str, value: Value) {
        Rc::get_mut(&mut self.bindings)
            .unwrap()
            .insert(String::from(name), value);
    }
    fn bind_fn(&mut self, name: &str, fun: &'static FunctionType) {
        self.bind_value(
            name,
            Value::Function(Function {
                name: String::from(name),
                fun: Rc::new(fun),
            }),
        );
    }
}

pub fn eval(ctx: &mut Context, value: Value) -> Result<Value, String> {
    match value {
        Value::Symbol(name) => {
            if let Some(val) = ctx.resolve(&name) {
                Ok(val)
            } else {
                Err(format!("Can't resolve symbol '{}'", name))
            }
        }
        Value::List(mut elements) => {
            match elements.first() {
                Some(Value::Symbol(name)) if name == "recur" => {
                    return Ok(Value::List(elements));
                }
                _ => {}
            };
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
