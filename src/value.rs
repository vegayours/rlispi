use im_lists::list::List;
use std::ops::Fn;
use std::rc::Rc;

use crate::eval::Context;

pub type FunctionType = dyn Fn(&mut Context, List<Value>) -> Result<Value, String>;

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub fun: Rc<FunctionType>,
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
pub enum Value {
    Bool(bool),
    Nil,
    Integer(i64),
    List(List<Value>),
    Function(Function),
    Symbol(String),
    String(String),
}
