use serde::{Deserialize, Serialize};
use worker::{
    workflow, Context, Env, NonRetryableError, Result, WorkflowEntrypoint, WorkflowEvent,
    WorkflowRetryConfig, WorkflowStep, WorkflowStepConfig,
};

#[derive(Debug, Deserialize)]
pub struct TestWorkflowInput {
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct TestWorkflowOutput {
    pub value: String,
}

#[workflow]
pub struct TestWorkflow;

impl WorkflowEntrypoint for TestWorkflow {
    type Input = TestWorkflowInput;
    type Output = TestWorkflowOutput;

    fn new(_ctx: Context, _env: Env) -> Self {
        Self
    }

    async fn run(
        &self,
        event: WorkflowEvent<Self::Input>,
        step: WorkflowStep,
    ) -> Result<Self::Output> {
        let value = event.payload.value;
        if value == "non-retryable" {
            let _: () = step
                .do_("terminal failure", |context| async move {
                    Err(NonRetryableError::new(format!(
                        "terminal failure on attempt {}",
                        context.attempt()
                    ))
                    .into())
                })
                .config(WorkflowStepConfig::new().retries(WorkflowRetryConfig::new(3, 1_u64)))
                .await?;
        }
        let value = step
            .do_("echo input", move |_| {
                let value = value.clone();
                async move { Ok(value) }
            })
            .await?;

        Ok(TestWorkflowOutput { value })
    }
}
