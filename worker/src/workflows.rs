//! Bindings for triggering and managing [Cloudflare Workflows](https://developers.cloudflare.com/workflows/).
//!
//! This module exposes the `[[workflows]]` binding available to a Worker. It
//! does not define Workflow entrypoint classes, which currently remain a
//! JavaScript runtime API.

use js_sys::{futures::JsFuture, Array};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use worker_sys::{Workflow as WorkflowSys, WorkflowInstance as WorkflowInstanceSys};

use crate::{EnvBinding, Result};

/// A binding to a Cloudflare Workflow.
#[derive(Debug, Clone)]
pub struct Workflow(WorkflowSys);

unsafe impl Send for Workflow {}
unsafe impl Sync for Workflow {}

impl EnvBinding for Workflow {
    const TYPE_NAME: &'static str = "Workflow";

    // Workflow bindings are interface values whose concrete constructor is an
    // implementation detail and differs between production and local dev.
    fn get(val: JsValue) -> Result<Self> {
        Ok(val.unchecked_into())
    }
}

impl JsCast for Workflow {
    fn instanceof(val: &JsValue) -> bool {
        val.is_object()
    }

    fn unchecked_from_js(val: JsValue) -> Self {
        Self(val.unchecked_into())
    }

    fn unchecked_from_js_ref(val: &JsValue) -> &Self {
        unsafe { &*(val as *const JsValue as *const Self) }
    }
}

impl AsRef<JsValue> for Workflow {
    fn as_ref(&self) -> &JsValue {
        &self.0
    }
}

impl From<Workflow> for JsValue {
    fn from(workflow: Workflow) -> Self {
        workflow.0.into()
    }
}

impl From<WorkflowSys> for Workflow {
    fn from(workflow: WorkflowSys) -> Self {
        Self(workflow)
    }
}

impl Workflow {
    /// Get a handle to an existing Workflow instance.
    pub async fn get(&self, id: impl AsRef<str>) -> Result<WorkflowInstance> {
        let value = JsFuture::from(self.0.get(id.as_ref())?).await?;
        Ok(WorkflowInstance(value.unchecked_into()))
    }

    /// Create a Workflow instance without an explicit ID or parameters.
    pub async fn create(&self) -> Result<WorkflowInstance> {
        self.create_inner(&JsValue::undefined()).await
    }

    /// Create a Workflow instance with an ID, parameters, or retention policy.
    pub async fn create_with_options<T: Serialize>(
        &self,
        options: &WorkflowInstanceCreateOptions<T>,
    ) -> Result<WorkflowInstance> {
        let options = serde_wasm_bindgen::to_value(options)?;
        self.create_inner(&options).await
    }

    async fn create_inner(&self, options: &JsValue) -> Result<WorkflowInstance> {
        let value = JsFuture::from(self.0.create(options)?).await?;
        Ok(WorkflowInstance(value.unchecked_into()))
    }

    /// Create multiple Workflow instances in one request.
    pub async fn create_batch<T: Serialize>(
        &self,
        batch: &[WorkflowInstanceCreateOptions<T>],
    ) -> Result<Vec<WorkflowInstance>> {
        let batch: Array = serde_wasm_bindgen::to_value(batch)?.unchecked_into();
        let value = JsFuture::from(self.0.create_batch(&batch)?).await?;
        let instances = Array::from(&value);

        Ok(instances
            .iter()
            .map(|value| WorkflowInstance(value.unchecked_into()))
            .collect())
    }

    /// Delete multiple Workflow instances and their stored state.
    pub async fn delete_batch<S: AsRef<str>>(
        &self,
        instance_ids: &[S],
    ) -> Result<WorkflowBatchDeleteResult> {
        let instance_ids = instance_ids
            .iter()
            .map(|id| JsValue::from_str(id.as_ref()))
            .collect::<Array>();
        let value = JsFuture::from(self.0.delete_batch(&instance_ids)?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}

/// Options for creating a Workflow instance.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInstanceCreateOptions<T = ()> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<WorkflowRetentionPolicy>,
}

impl<T> Default for WorkflowInstanceCreateOptions<T> {
    fn default() -> Self {
        Self {
            id: None,
            params: None,
            retention: None,
        }
    }
}

impl WorkflowInstanceCreateOptions<()> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T> WorkflowInstanceCreateOptions<T> {
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the parameters, changing the options' parameter type in the process.
    #[must_use]
    pub fn params<U>(self, params: U) -> WorkflowInstanceCreateOptions<U> {
        WorkflowInstanceCreateOptions {
            id: self.id,
            params: Some(params),
            retention: self.retention,
        }
    }

    #[must_use]
    pub fn retention(mut self, retention: WorkflowRetentionPolicy) -> Self {
        self.retention = Some(retention);
        self
    }
}

/// How long to retain a Workflow instance after it reaches a terminal state.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRetentionPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_retention: Option<WorkflowDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_retention: Option<WorkflowDuration>,
}

impl WorkflowRetentionPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn success(mut self, duration: impl Into<WorkflowDuration>) -> Self {
        self.success_retention = Some(duration.into());
        self
    }

    #[must_use]
    pub fn error(mut self, duration: impl Into<WorkflowDuration>) -> Self {
        self.error_retention = Some(duration.into());
        self
    }
}

/// A duration accepted by the Workflows API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowDuration {
    Milliseconds(u64),
    Expression(String),
}

impl From<u64> for WorkflowDuration {
    fn from(milliseconds: u64) -> Self {
        Self::Milliseconds(milliseconds)
    }
}

impl From<String> for WorkflowDuration {
    fn from(expression: String) -> Self {
        Self::Expression(expression)
    }
}

impl From<&str> for WorkflowDuration {
    fn from(expression: &str) -> Self {
        Self::Expression(expression.to_owned())
    }
}

/// The result of deleting a batch of Workflow instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowBatchDeleteResult {
    pub deleted: Vec<WorkflowBatchDeletedInstance>,
    pub errors: Vec<WorkflowBatchDeleteError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowBatchDeletedInstance {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowBatchDeleteError {
    pub id: String,
    pub code: u32,
    pub message: String,
}

/// A handle to one Workflow instance.
#[derive(Debug, Clone)]
pub struct WorkflowInstance(WorkflowInstanceSys);

unsafe impl Send for WorkflowInstance {}
unsafe impl Sync for WorkflowInstance {}

impl JsCast for WorkflowInstance {
    fn instanceof(val: &JsValue) -> bool {
        val.is_object()
    }

    fn unchecked_from_js(val: JsValue) -> Self {
        Self(val.unchecked_into())
    }

    fn unchecked_from_js_ref(val: &JsValue) -> &Self {
        unsafe { &*(val as *const JsValue as *const Self) }
    }
}

impl AsRef<JsValue> for WorkflowInstance {
    fn as_ref(&self) -> &JsValue {
        &self.0
    }
}

impl From<WorkflowInstance> for JsValue {
    fn from(instance: WorkflowInstance) -> Self {
        instance.0.into()
    }
}

impl From<WorkflowInstanceSys> for WorkflowInstance {
    fn from(instance: WorkflowInstanceSys) -> Self {
        Self(instance)
    }
}

impl WorkflowInstance {
    pub fn id(&self) -> String {
        self.0.id()
    }

    pub async fn pause(&self) -> Result<()> {
        JsFuture::from(self.0.pause()?).await?;
        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        JsFuture::from(self.0.resume()?).await?;
        Ok(())
    }

    /// Terminate this instance without running registered rollback handlers.
    pub async fn terminate(&self) -> Result<()> {
        self.terminate_inner(&JsValue::undefined()).await
    }

    /// Terminate this instance with explicit rollback behavior.
    pub async fn terminate_with_options(
        &self,
        options: &WorkflowInstanceTerminateOptions,
    ) -> Result<()> {
        let options = serde_wasm_bindgen::to_value(options)?;
        self.terminate_inner(&options).await
    }

    async fn terminate_inner(&self, options: &JsValue) -> Result<()> {
        JsFuture::from(self.0.terminate(options)?).await?;
        Ok(())
    }

    /// Restart this instance from the beginning.
    pub async fn restart(&self) -> Result<()> {
        self.restart_inner(&JsValue::undefined()).await
    }

    /// Restart this instance from a specific step.
    pub async fn restart_with_options(
        &self,
        options: &WorkflowInstanceRestartOptions,
    ) -> Result<()> {
        let options = serde_wasm_bindgen::to_value(options)?;
        self.restart_inner(&options).await
    }

    async fn restart_inner(&self, options: &JsValue) -> Result<()> {
        JsFuture::from(self.0.restart(options)?).await?;
        Ok(())
    }

    /// Delete this instance and its stored state.
    pub async fn delete(&self) -> Result<()> {
        JsFuture::from(self.0.delete()?).await?;
        Ok(())
    }

    /// Return the current status, retaining the output as a raw JavaScript
    /// value so structured-clone types are not lost.
    pub async fn status(&self) -> Result<WorkflowInstanceStatus> {
        let value = JsFuture::from(self.0.status()?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    /// Send an event to a matching `waitForEvent` step.
    pub async fn send_event<T: Serialize>(
        &self,
        event_type: impl AsRef<str>,
        payload: &T,
    ) -> Result<()> {
        let event = WorkflowInstanceEvent {
            event_type: event_type.as_ref(),
            payload,
        };
        let event = serde_wasm_bindgen::to_value(&event)?;
        JsFuture::from(self.0.send_event(&event)?).await?;
        Ok(())
    }
}

#[derive(Serialize)]
struct WorkflowInstanceEvent<'a, T> {
    #[serde(rename = "type")]
    event_type: &'a str,
    payload: &'a T,
}

/// Options controlling Workflow instance termination.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkflowInstanceTerminateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback: Option<bool>,
}

impl WorkflowInstanceTerminateOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn rollback(mut self, rollback: bool) -> Self {
        self.rollback = Some(rollback);
        self
    }
}

/// Options controlling where a Workflow instance restarts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkflowInstanceRestartOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<WorkflowRestartFrom>,
}

impl WorkflowInstanceRestartOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from(mut self, step: WorkflowRestartFrom) -> Self {
        self.from = Some(step);
        self
    }
}

/// Identifies a step from which to restart a Workflow instance.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRestartFrom {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub step_type: Option<WorkflowStepType>,
}

impl WorkflowRestartFrom {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            count: None,
            step_type: None,
        }
    }

    #[must_use]
    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    #[must_use]
    pub fn step_type(mut self, step_type: WorkflowStepType) -> Self {
        self.step_type = Some(step_type);
        self
    }
}

/// A Workflow step type used to disambiguate restart targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowStepType {
    Do,
    Sleep,
    WaitForEvent,
}

/// Current state and output of a Workflow instance.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowInstanceStatus {
    pub status: WorkflowStatus,
    #[serde(default)]
    pub error: Option<WorkflowError>,
    #[serde(default, deserialize_with = "deserialize_optional_js_value")]
    pub output: Option<JsValue>,
    #[serde(default)]
    pub rollback: Option<WorkflowRollbackStatus>,
}

impl WorkflowInstanceStatus {
    /// Deserialize the persisted Workflow output into a Rust type.
    pub fn output<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        self.output
            .as_ref()
            .map(|output| serde_wasm_bindgen::from_value(output.clone()).map_err(Into::into))
            .transpose()
    }
}

fn deserialize_optional_js_value<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<JsValue>, D::Error>
where
    D: Deserializer<'de>,
{
    serde_wasm_bindgen::preserve::deserialize(deserializer).map(Some)
}

/// Lifecycle state of a Workflow instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowStatus {
    Queued,
    Running,
    Paused,
    Errored,
    Terminated,
    Complete,
    Waiting,
    WaitingForPause,
    #[serde(other)]
    Unknown,
}

/// Error reported by a Workflow instance or rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowError {
    pub name: String,
    pub message: String,
}

/// Result of registered rollback handlers after a Workflow reaches a terminal
/// state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRollbackStatus {
    pub outcome: WorkflowRollbackOutcome,
    pub error: Option<WorkflowError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowRollbackOutcome {
    Complete,
    Failed,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Serialize)]
    struct Params {
        user_id: String,
    }

    #[test]
    fn create_options_match_the_runtime_shape() {
        let options = WorkflowInstanceCreateOptions::new()
            .id("instance-1")
            .retention(
                WorkflowRetentionPolicy::new()
                    .success("1 day")
                    .error(86_400_u64),
            )
            .params(Params {
                user_id: "user-1".into(),
            });

        assert_eq!(
            serde_json::to_value(options).unwrap(),
            json!({
                "id": "instance-1",
                "params": { "user_id": "user-1" },
                "retention": {
                    "successRetention": "1 day",
                    "errorRetention": 86400
                }
            })
        );
    }

    #[test]
    fn restart_options_match_the_runtime_shape() {
        let options = WorkflowInstanceRestartOptions::new().from(
            WorkflowRestartFrom::new("process")
                .count(3)
                .step_type(WorkflowStepType::WaitForEvent),
        );

        assert_eq!(
            serde_json::to_value(options).unwrap(),
            json!({
                "from": {
                    "name": "process",
                    "count": 3,
                    "type": "waitForEvent"
                }
            })
        );
    }
}
