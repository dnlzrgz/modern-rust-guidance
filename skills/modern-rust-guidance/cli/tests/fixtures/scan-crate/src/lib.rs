/// Manual `let … else` pattern that `clippy::manual_let_else` reports.
pub fn first(x: Option<u32>) -> u32 {
    let v = match x {
        Some(v) => v,
        None => return 0,
    };
    v + 1
}
