use crate::{AbacusError, Value};

#[derive(Debug)]
pub struct BinaryOp {
    pub alias: &'static str,
    pub func: fn(Value, Value) -> Result<Value, AbacusError>,
    pub precedence: u8,
    pub right_associative: bool,
}

impl BinaryOp {
    pub fn apply(&self, lhs: Value, rhs: Value) -> Result<Value, AbacusError> {
        (self.func)(lhs, rhs)
    }
}
