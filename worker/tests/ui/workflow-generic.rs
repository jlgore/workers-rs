use worker::workflow;

#[workflow]
pub struct GenericWorkflow<T>(T);

fn main() {}
