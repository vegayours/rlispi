use crate::value::Value;
use std::collections::LinkedList;

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

pub struct Parser {
    state: LinkedList<Value>,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            state: LinkedList::new(),
        }
    }
    pub fn parse_next(&mut self, src: &str) -> Result<Vec<Value>, String> {
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
    pub fn finish(self) -> Result<(), String> {
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
