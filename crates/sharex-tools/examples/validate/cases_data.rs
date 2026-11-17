//! The Data, Video and System tools' cases.

use serde_json::Value;

use crate::cases::Ctx;
use crate::compare::Outcome;

pub fn run(_ctx: &Ctx, _c: &Value, tool: &str) -> Outcome {
    Outcome::skip(format!("no validator for {tool}"))
}
