use serde_json::Value;

pub fn render_qasm(target: &Value) -> String {
    crate::render_baseline_qasm(target)
}
