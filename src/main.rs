use std::collections::LinkedList;
use std::ops::Fn;
use std::rc::Rc;
use std::{collections::HashMap, io::Read};

struct TokenIterator<'a> {
    src: &'a str,
}

impl<'a> TokenIterator<'a> {
    fn next_token(&mut self) -> Result<Option<&'a str>, String> {
        self.src = self.src.trim_start();

        if self.src.is_empty() {
            return Ok(None);
        }

        match &self.src {
            p if p.starts_with('(') || p.starts_with(')') => {
                let token = &self.src[..1];
                self.src = &self.src[1..];
                Ok(Some(token))
            }
            /*
            s if s.starts_with('"') => {
                if let Some((pos, _)) = *&self.src.match_indices('"').skip(1).next() {
                    let token = &self.src[..pos+1];
                    self.src = &self.src[pos+1..];
                    Ok(Some(token))
                } else {
                    Err(String::from("String is not terminated"))
                }
            },
            */
            _ => match &self.src.find(|x: char| x.is_ascii_whitespace() || x == ')') {
                Some(pos) => {
                    let token = &self.src[..*pos];
                    self.src = &self.src[*pos..];
                    Ok(Some(token))
                }
                None => {
                    let token = self.src;
                    self.src = "";
                    Ok(Some(token))
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
enum Node {
    List { elements: Vec<Node> },
    Symbol { name: String },
    Integer(i64),
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

struct NodeParser {
    nodes_stack: Vec<Node>
}

impl Node {
    fn parse(src: &str) -> Result<Node, String> {
        let mut nodes_stack: Vec<Node> = Vec::new();
        nodes_stack.push(Node::List {
            elements: Vec::new(),
        });

        fn add_node(node: Node, nodes_stack: &mut Vec<Node>) -> Result<(), String> {
            match nodes_stack.last_mut() {
                Some(Node::List { elements }) => {
                    elements.push(node);
                    Ok(())
                }
                Some(_) => {
                    Err(String::from("Trying to add node to non-List node"))
                }
                None => {
                    Err(String::from("Unbalanced ')' parenthesis"))
                }
            }
        }

        let mut token_iter = TokenIterator { src };
        while let Some(token) = token_iter.next_token()? {
            match token {
                "(" => {
                    nodes_stack.push(Node::List {
                        elements: Vec::new(),
                    });
                }
                ")" => {
                    let last = nodes_stack.pop();
                    match last {
                        Some(node @ Node::List { .. }) => {
                            add_node(node, &mut nodes_stack)?;
                        }
                        _ => {
                            return Err(String::from("Unbalanced closing parenthesis"));
                        }
                    }
                }
                _ => {
                    if let Ok(i64_value) = token.parse::<i64>() {
                        add_node(Node::Integer(i64_value), &mut nodes_stack)?;
                    } else if is_symbol(token) {
                        add_node(
                            Node::Symbol {
                                name: String::from(token),
                            },
                            &mut nodes_stack,
                        )?;
                    } else {
                        return Err(format!("Can't parse token: {}", token));
                    }
                }
            }
        }

        if nodes_stack.len() > 1usize {
            return Err(String::from("Unbalanced open parenthesis"));
        }

        match nodes_stack.pop() {
            Some(node) => Ok(node),
            None => Err(String::from("No node was parsed")),
        }
    }
}

#[derive(Clone)]
struct Function {
    name: String,
    fun: Rc<dyn Fn(&mut Context, &[Node]) -> Result<Value, String>>,
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
}

#[derive(Default, Clone, Debug)]
struct Context {
    bindings: HashMap<String, Value>,
    local: HashMap<String, Value>
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
                fun: Rc::new(|ctx, args| {
                    if args.is_empty() {
                        return Err("Function '=' called without arguments".into());
                    }
                    let value = eval(ctx, args.first().unwrap())?;
                    for other in &args[1..] {
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
                        if let Value::Integer(value) = eval(ctx, arg)? {
                            result += value;
                        } else {
                            return Err(format!("Calling function '+' with arg: {:?}", arg));
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
                fun: Rc::new(|ctx, args| match args {
                    [Node::Symbol { name }, node] => {
                        let value = eval(ctx, node)?;
                        ctx.bindings.insert(name.clone(), value);
                        Ok(Value::Nil)
                    }
                    _ => Err(format!("Invalid arguments for def: {:?}", args)),
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
                fun: Rc::new(|ctx, args| {
                    if args.len() != 1 {
                        return Err("Function 'first' requires 1 argument".into());
                    }
                    if let Value::List(mut elements) = eval(ctx, args.first().unwrap())? {
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
                fun: Rc::new(|ctx, args| {
                    if args.len() != 1 {
                        return Err("Function 'rest' requires 1 argument".into());
                    }
                    if let Value::List(mut elements) = eval(ctx, args.first().unwrap())? {
                        match elements.pop_front() {
                            Some(_) => Ok(Value::List(elements)),
                            None => Err("Function 'rest' requires non-empty list".into()),
                        }
                    } else {
                        Err("Only list is supported for 'rest' function".into())
                    }
                }),
            }),
        );
        bindings.insert(
            "cons".into(),
            Value::Function(Function {
                name: "cons".into(),
                fun: Rc::new(|ctx, args| {
                    if args.len() != 2 {
                        return Err("Function 'cons' requires 2 arguments".into());
                    }
                    let elem = eval(ctx, args.first().unwrap())?;
                    if let Value::List(mut elements) = eval(ctx, args.iter().skip(1).next().unwrap())? {
                        elements.push_front(elem);
                        Ok(Value::List(elements))
                    } else {
                        Err("Only list is supported for 'cons' function 2nd argument".into())
                    }
                }),
            }),
        );
        bindings.insert(
            "empty?".into(),
            Value::Function(Function {
                name: "empty?".into(),
                fun: Rc::new(|ctx, args| {
                    if args.len() != 1 {
                        return Err("Function 'empty' requires 1 argument".into());
                    }
                    if let Value::List(elements) = eval(ctx, args.first().unwrap())? {
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
                fun: Rc::new(|ctx, args| {
                    let (cond_node, true_node, false_node) = match args {
                        [cond_node, true_node] => (cond_node, true_node, None),
                        [cond_node, true_node, false_node] => {
                            (cond_node, true_node, Some(false_node))
                        }
                        _ => {
                            return Err("Function 'if' requires 2 or 3 arguments".into());
                        }
                    };
                    match eval(ctx, cond_node)? {
                        Value::Bool(false) | Value::Nil => {
                            false_node.map_or(Ok(Value::Nil), |node| eval(ctx, node))
                        }
                        _ => eval(ctx, true_node),
                    }
                }),
            }),
        );
        bindings.insert(
            "fn".into(),
            Value::Function(Function {
                name: "fn".into(),
                fun: Rc::new(|ctx, args| {
                    if let [Node::List {elements}, body] = args {
                        let mut bindings: Vec<String> = Vec::new();
                        for arg_binding in elements {
                            if let Node::Symbol{name} = arg_binding {
                                bindings.push(name.clone());
                            } else {
                                return Err(format!("Function arguments must be symbols, got {:?}.", arg_binding));
                            }
                        }
                        let body_copy = body.clone();
                        let local_context = ctx.local.clone();

                        let f = move |global_ctx: &mut Context, args: &[Node]| -> Result<Value, String> {
                            if bindings.len() != args.len() {
                                return Err(format!("Wrong number of arguments, expected {}, got {}", bindings.len(), args.len()));
                            }
                            let mut inner_ctx = Context { bindings: global_ctx.bindings.clone(), local: local_context.clone() };
                            for (name, bound_node) in bindings.iter().zip(args) {
                                let bound_value = eval(&mut inner_ctx, bound_node)?;
                                inner_ctx.local.insert(name.clone(), bound_value);
                            }
                            let result = eval(&mut inner_ctx, &body_copy)?;
                            Ok(result)
                        };
                        Ok(Value::Function(Function {
                            name: "anonymous".into(),
                            fun: Rc::new(f)
                        }))
                    }
                    else {
                        Err("Function 'fn' has form (fn (arg1 arg2 ...) body)".into())
                    }
                }),
            }),
        );
        Context { bindings , ..Default::default() }
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

fn eval(ctx: &mut Context, node: &Node) -> Result<Value, String> {
    println!("Node: {:?}. Local ctx: {:?}", node, ctx.local);
    match node {
        Node::Symbol { name } => {
            if let Some(val) = ctx.resolve(name) {
                println!("{} -> {:?}", name, val);
                Ok(val)
            } else {
                Err(format!("Can't resolve symbol '{}'", name))
            }
        },
        Node::Integer(value) => Ok(Value::Integer(*value)),
        Node::List { elements } => match elements.first() {
            Some(node) => match eval(ctx, node)? {
                Value::Function(Function { fun, ..}) => fun(ctx, &elements[1..]),
                other => Err(format!(
                    "Node {:?} value is not a function, but is '{:?}'",
                    node, other
                )),
            },
            None => Err(format!("Can't evaluate empty list")),
        },
    }
}

fn main() {
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();

    let node = Node::parse(&src).unwrap();

    println!("Src: {}", src.trim_end());
    println!("Overall node: {:?}", node);

    let mut context = Context::new();
    if let Node::List { elements } = node {
        for elem in &elements {
            println!("Eval {:?}", elem);
            let result = eval(&mut context, elem).unwrap();
            println!("Result {:?}", result);
        }
    } else {
        panic!("Top-level node must be a list");
    }
}
