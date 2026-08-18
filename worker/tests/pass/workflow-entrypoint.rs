use serde::{Deserialize, Serialize};
use worker::{workflow, Context, Env, Result, WorkflowEntrypoint, WorkflowEvent, WorkflowStep};

#[derive(Deserialize)]
pub struct Input {
    value: String,
}

#[derive(Serialize)]
pub struct Output {
    value: String,
}

#[workflow]
pub struct ExampleWorkflow;

impl WorkflowEntrypoint for ExampleWorkflow {
    type Input = Input;
    type Output = Output;

    fn new(_ctx: Context, _env: Env) -> Self {
        Self
    }

    async fn run(
        &self,
        event: WorkflowEvent<Self::Input>,
        step: WorkflowStep,
    ) -> Result<Self::Output> {
        let value = event.payload.value;
        let value = step
            .do_("echo", move |_| {
                let value = value.clone();
                async move { Ok(value) }
            })
            .await?;
        Ok(Output { value })
    }
}

fn main() {}
