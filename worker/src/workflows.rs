//! Bindings for triggering and managing [Cloudflare Workflows](https://developers.cloudflare.com/workflows/).
//!
//! This module exposes both the `[[workflows]]` binding used to manage Workflow
//! instances and the types used to define Workflow entrypoints in Rust.

use std::{
    cell::RefCell,
    fmt,
    future::{Future, IntoFuture},
    marker::PhantomData,
    panic::AssertUnwindSafe,
    pin::Pin,
    rc::Rc,
};

use js_sys::{
    futures::{future_to_promise, JsFuture},
    Array, Function, Promise, Reflect,
};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use worker_sys::{
    NonRetryableError as NonRetryableErrorSys, Workflow as WorkflowSys,
    WorkflowInstance as WorkflowInstanceSys, WorkflowStep as WorkflowStepSys,
};

use crate::{Context, Date, Env, EnvBinding, Error, Result};

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

/// Metadata passed to a Workflow when an instance is created by a cron schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCronSchedule {
    pub cron: String,
    pub scheduled_time: u64,
}

/// The immutable event supplied to [`WorkflowEntrypoint::run`].
#[derive(Debug, Clone)]
pub struct WorkflowEvent<T> {
    pub payload: T,
    pub timestamp: Date,
    pub instance_id: String,
    /// The configured Workflow name, or an empty string when an older local
    /// runtime does not provide this field.
    pub workflow_name: String,
    pub schedule: Option<WorkflowCronSchedule>,
}

impl<T: DeserializeOwned> WorkflowEvent<T> {
    #[doc(hidden)]
    pub fn _from_raw(raw: JsValue) -> Result<Self> {
        let payload =
            serde_wasm_bindgen::from_value(Reflect::get(&raw, &JsValue::from_str("payload"))?)?;
        let timestamp = Reflect::get(&raw, &JsValue::from_str("timestamp"))?
            .dyn_into::<js_sys::Date>()?
            .into();
        let instance_id = required_workflow_string(&raw, "instanceId")?;
        let workflow_name = Reflect::get(&raw, &JsValue::from_str("workflowName"))?
            .as_string()
            .unwrap_or_default();
        let schedule = Reflect::get(&raw, &JsValue::from_str("schedule"))?;
        let schedule = (!schedule.is_null_or_undefined())
            .then(|| serde_wasm_bindgen::from_value(schedule))
            .transpose()?;

        Ok(Self {
            payload,
            timestamp,
            instance_id,
            workflow_name,
            schedule,
        })
    }
}

fn required_workflow_string(raw: &JsValue, property: &str) -> Result<String> {
    Reflect::get(raw, &JsValue::from_str(property))?
        .as_string()
        .ok_or_else(|| Error::RustError(format!("Workflow event `{property}` must be a string")))
}

/// An event delivered by [`WorkflowStep::wait_for_event`].
#[derive(Debug, Clone)]
pub struct WorkflowStepEvent<T> {
    pub payload: T,
    pub timestamp: Date,
    pub event_type: String,
    pub sensitive: Option<WorkflowStepSensitivity>,
}

impl<T: DeserializeOwned> WorkflowStepEvent<T> {
    fn from_raw(raw: JsValue) -> Result<Self> {
        let payload =
            serde_wasm_bindgen::from_value(Reflect::get(&raw, &JsValue::from_str("payload"))?)?;
        let timestamp = Reflect::get(&raw, &JsValue::from_str("timestamp"))?
            .dyn_into::<js_sys::Date>()?
            .into();
        let event_type = required_workflow_string(&raw, "type")?;
        let sensitive = Reflect::get(&raw, &JsValue::from_str("sensitive"))?;
        let sensitive = (!sensitive.is_null_or_undefined())
            .then(|| serde_wasm_bindgen::from_value(sensitive))
            .transpose()?;

        Ok(Self {
            payload,
            timestamp,
            event_type,
            sensitive,
        })
    }
}

/// Backoff algorithm applied between Workflow step attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowBackoff {
    Constant,
    Linear,
    Exponential,
}

/// Marks persisted Workflow step output as sensitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowStepSensitivity {
    Output,
}

/// Retry policy for a Workflow step.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRetryConfig {
    limit: u32,
    delay: WorkflowDuration,
    #[serde(skip_serializing_if = "Option::is_none")]
    backoff: Option<WorkflowBackoff>,
}

impl WorkflowRetryConfig {
    pub fn new(limit: u32, delay: impl Into<WorkflowDuration>) -> Self {
        Self {
            limit,
            delay: delay.into(),
            backoff: None,
        }
    }

    #[must_use]
    pub fn backoff(mut self, backoff: WorkflowBackoff) -> Self {
        self.backoff = Some(backoff);
        self
    }
}

/// Configuration applied to a Workflow step.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkflowStepConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    retries: Option<WorkflowRetryConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<WorkflowDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sensitive: Option<WorkflowStepSensitivity>,
}

impl WorkflowStepConfig {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn retries(mut self, retries: WorkflowRetryConfig) -> Self {
        self.retries = Some(retries);
        self
    }

    #[must_use]
    pub fn timeout(mut self, timeout: impl Into<WorkflowDuration>) -> Self {
        self.timeout = Some(timeout.into());
        self
    }

    #[must_use]
    pub fn sensitive_output(mut self) -> Self {
        self.sensitive = Some(WorkflowStepSensitivity::Output);
        self
    }
}

/// Resolved retry policy reported to a Workflow step callback.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkflowResolvedRetryConfig {
    pub limit: u32,
    #[serde(default)]
    pub delay: Option<WorkflowDuration>,
    #[serde(default)]
    pub backoff: Option<WorkflowBackoff>,
}

/// Resolved configuration reported to a Workflow step callback.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct WorkflowResolvedStepConfig {
    #[serde(default)]
    pub retries: Option<WorkflowResolvedRetryConfig>,
    #[serde(default)]
    pub timeout: Option<WorkflowDuration>,
    #[serde(default)]
    pub sensitive: Option<WorkflowStepSensitivity>,
}

#[derive(Deserialize)]
struct WorkflowStepInfo {
    name: String,
    count: u32,
}

#[derive(Deserialize)]
struct WorkflowStepContextWire {
    step: WorkflowStepInfo,
    attempt: u32,
    #[serde(default)]
    config: WorkflowResolvedStepConfig,
}

/// Runtime information supplied to each `do_` callback.
#[derive(Debug, Clone)]
pub struct WorkflowStepContext {
    name: String,
    count: u32,
    attempt: u32,
    config: WorkflowResolvedStepConfig,
}

impl WorkflowStepContext {
    fn from_raw(raw: JsValue) -> Result<Self> {
        let wire: WorkflowStepContextWire = serde_wasm_bindgen::from_value(raw)?;
        Ok(Self {
            name: wire.step.name,
            count: wire.step.count,
            attempt: wire.attempt,
            config: wire.config,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn config(&self) -> &WorkflowResolvedStepConfig {
        &self.config
    }
}

/// Options for a Workflow step waiting on an external event.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowWaitForEventOptions {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<WorkflowDuration>,
}

impl WorkflowWaitForEventOptions {
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            timeout: None,
        }
    }

    #[must_use]
    pub fn timeout(mut self, timeout: impl Into<WorkflowDuration>) -> Self {
        self.timeout = Some(timeout.into());
        self
    }
}

/// Timestamp accepted by [`WorkflowStep::sleep_until`].
#[derive(Debug, Clone)]
pub enum WorkflowTimestamp {
    Milliseconds(u64),
    Date(Date),
}

impl From<u64> for WorkflowTimestamp {
    fn from(milliseconds: u64) -> Self {
        Self::Milliseconds(milliseconds)
    }
}

impl From<Date> for WorkflowTimestamp {
    fn from(date: Date) -> Self {
        Self::Date(date)
    }
}

impl WorkflowTimestamp {
    fn into_js(self) -> JsValue {
        match self {
            Self::Milliseconds(milliseconds) => JsValue::from_f64(milliseconds as f64),
            Self::Date(date) => js_sys::Date::from(date).into(),
        }
    }
}

/// The durable step interface supplied to a Workflow entrypoint.
#[derive(Debug, Clone)]
pub struct WorkflowStep(WorkflowStepSys);

unsafe impl Send for WorkflowStep {}
unsafe impl Sync for WorkflowStep {}

impl From<WorkflowStepSys> for WorkflowStep {
    fn from(step: WorkflowStepSys) -> Self {
        Self(step)
    }
}

impl WorkflowStep {
    /// Define a named, retriable step whose serializable output is persisted.
    pub fn do_<T, F, Fut>(
        &self,
        name: impl Into<String>,
        callback: F,
    ) -> WorkflowStepCall<T, F, Fut>
    where
        T: Serialize + DeserializeOwned + 'static,
        F: FnMut(WorkflowStepContext) -> Fut + 'static,
        Fut: Future<Output = Result<T>> + 'static,
    {
        WorkflowStepCall {
            step: self.clone(),
            name: name.into(),
            callback,
            config: None,
            marker: PhantomData,
        }
    }

    /// Define a named step that exchanges raw JavaScript values with the
    /// Workflows runtime.
    ///
    /// This is the escape hatch for structured-clone values that cannot be
    /// represented through Serde, such as streams.
    pub fn do_raw<F, Fut>(
        &self,
        name: impl Into<String>,
        callback: F,
    ) -> RawWorkflowStepCall<F, Fut>
    where
        F: FnMut(WorkflowStepContext) -> Fut + 'static,
        Fut: Future<Output = Result<JsValue>> + 'static,
    {
        RawWorkflowStepCall {
            step: self.clone(),
            name: name.into(),
            callback,
            config: None,
            marker: PhantomData,
        }
    }

    pub async fn sleep(
        &self,
        name: impl AsRef<str>,
        duration: impl Into<WorkflowDuration>,
    ) -> Result<()> {
        let duration = serde_wasm_bindgen::to_value(&duration.into())?;
        JsFuture::from(self.0.sleep(name.as_ref(), &duration)?)
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn sleep_until(
        &self,
        name: impl AsRef<str>,
        timestamp: impl Into<WorkflowTimestamp>,
    ) -> Result<()> {
        JsFuture::from(
            self.0
                .sleep_until(name.as_ref(), &timestamp.into().into_js())?,
        )
        .await
        .map(|_| ())
        .map_err(Into::into)
    }

    pub async fn wait_for_event<T: DeserializeOwned>(
        &self,
        name: impl AsRef<str>,
        options: WorkflowWaitForEventOptions,
    ) -> Result<WorkflowStepEvent<T>> {
        let options = serde_wasm_bindgen::to_value(&options)?;
        let event = JsFuture::from(self.0.wait_for_event(name.as_ref(), &options)?).await?;
        WorkflowStepEvent::from_raw(event)
    }
}

/// Configurable future returned by [`WorkflowStep::do_`].
#[must_use = "Workflow steps must be awaited"]
pub struct WorkflowStepCall<T, F, Fut> {
    step: WorkflowStep,
    name: String,
    callback: F,
    config: Option<WorkflowStepConfig>,
    marker: PhantomData<fn() -> (T, Fut)>,
}

impl<T, F, Fut> fmt::Debug for WorkflowStepCall<T, F, Fut> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkflowStepCall")
            .field("name", &self.name)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<T, F, Fut> WorkflowStepCall<T, F, Fut> {
    pub fn config(mut self, config: WorkflowStepConfig) -> Self {
        self.config = Some(config);
        self
    }
}

impl<T, F, Fut> WorkflowStepCall<T, F, Fut>
where
    T: Serialize + DeserializeOwned + 'static,
    F: FnMut(WorkflowStepContext) -> Fut + 'static,
    Fut: Future<Output = Result<T>> + 'static,
{
    async fn execute(self) -> Result<T> {
        let callback = Rc::new(RefCell::new(self.callback));
        let js_callback = Closure::wrap(Box::new(move |raw_context: JsValue| -> Promise {
            let context = match WorkflowStepContext::from_raw(raw_context) {
                Ok(context) => context,
                Err(error) => return Promise::reject(&JsValue::from(error)),
            };
            let future = (callback.borrow_mut())(context);

            future_to_promise(AssertUnwindSafe(async move {
                let output = future.await.map_err(JsValue::from)?;
                serde_wasm_bindgen::to_value(&output)
                    .map_err(Error::from)
                    .map_err(JsValue::from)
            }))
        }) as Box<dyn FnMut(JsValue) -> Promise>);
        let function: &Function = js_callback.as_ref().unchecked_ref();
        let promise = match self.config {
            Some(config) => {
                let config = serde_wasm_bindgen::to_value(&config)?;
                self.step.0.do_with_config(&self.name, &config, function)?
            }
            None => self.step.0.do_(&self.name, function)?,
        };
        let output = JsFuture::from(promise).await?;
        drop(js_callback);
        Ok(serde_wasm_bindgen::from_value(output)?)
    }
}

impl<T, F, Fut> IntoFuture for WorkflowStepCall<T, F, Fut>
where
    T: Serialize + DeserializeOwned + 'static,
    F: FnMut(WorkflowStepContext) -> Fut + 'static,
    Fut: Future<Output = Result<T>> + 'static,
{
    type Output = Result<T>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output>>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

/// Configurable future returned by [`WorkflowStep::do_raw`].
#[must_use = "Workflow steps must be awaited"]
pub struct RawWorkflowStepCall<F, Fut> {
    step: WorkflowStep,
    name: String,
    callback: F,
    config: Option<WorkflowStepConfig>,
    marker: PhantomData<fn() -> Fut>,
}

impl<F, Fut> fmt::Debug for RawWorkflowStepCall<F, Fut> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawWorkflowStepCall")
            .field("name", &self.name)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<F, Fut> RawWorkflowStepCall<F, Fut> {
    pub fn config(mut self, config: WorkflowStepConfig) -> Self {
        self.config = Some(config);
        self
    }
}

impl<F, Fut> RawWorkflowStepCall<F, Fut>
where
    F: FnMut(WorkflowStepContext) -> Fut + 'static,
    Fut: Future<Output = Result<JsValue>> + 'static,
{
    async fn execute(self) -> Result<JsValue> {
        let callback = Rc::new(RefCell::new(self.callback));
        let js_callback = Closure::wrap(Box::new(move |raw_context: JsValue| -> Promise {
            let context = match WorkflowStepContext::from_raw(raw_context) {
                Ok(context) => context,
                Err(error) => return Promise::reject(&JsValue::from(error)),
            };
            let future = (callback.borrow_mut())(context);

            future_to_promise(AssertUnwindSafe(async move {
                future.await.map_err(JsValue::from)
            }))
        }) as Box<dyn FnMut(JsValue) -> Promise>);
        let function: &Function = js_callback.as_ref().unchecked_ref();
        let promise = match self.config {
            Some(config) => {
                let config = serde_wasm_bindgen::to_value(&config)?;
                self.step.0.do_with_config(&self.name, &config, function)?
            }
            None => self.step.0.do_(&self.name, function)?,
        };
        let output = JsFuture::from(promise).await?;
        drop(js_callback);
        Ok(output)
    }
}

impl<F, Fut> IntoFuture for RawWorkflowStepCall<F, Fut>
where
    F: FnMut(WorkflowStepContext) -> Fut + 'static,
    Fut: Future<Output = Result<JsValue>> + 'static,
{
    type Output = Result<JsValue>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output>>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.execute())
    }
}

/// A terminal Workflow error which prevents a failed step from being retried.
#[derive(Debug, Clone)]
pub struct NonRetryableError(NonRetryableErrorSys);

impl NonRetryableError {
    pub fn new(message: impl AsRef<str>) -> Self {
        Self(NonRetryableErrorSys::new(message.as_ref()))
    }
}

impl From<NonRetryableError> for Error {
    fn from(error: NonRetryableError) -> Self {
        Self::Internal(error.0.into())
    }
}

/// Implemented by Rust types exported as Cloudflare Workflow entrypoints.
///
/// The implementing struct must also carry the [`workflow`](crate::workflow)
/// attribute so `worker-build` can generate the JavaScript runtime subclass.
#[allow(async_fn_in_trait)]
pub trait WorkflowEntrypoint: has_workflow_attribute + Sized {
    type Input: DeserializeOwned + 'static;
    type Output: Serialize + 'static;

    fn new(ctx: Context, env: Env) -> Self;

    async fn run(
        &self,
        event: WorkflowEvent<Self::Input>,
        step: WorkflowStep,
    ) -> Result<Self::Output>;
}

#[doc(hidden)]
#[allow(non_camel_case_types)]
pub trait has_workflow_attribute {}

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

    #[test]
    fn step_config_matches_the_runtime_shape() {
        let config = WorkflowStepConfig::new()
            .retries(
                WorkflowRetryConfig::new(5, "10 seconds").backoff(WorkflowBackoff::Exponential),
            )
            .timeout("2 minutes")
            .sensitive_output();

        assert_eq!(
            serde_json::to_value(config).unwrap(),
            json!({
                "retries": {
                    "limit": 5,
                    "delay": "10 seconds",
                    "backoff": "exponential"
                },
                "timeout": "2 minutes",
                "sensitive": "output"
            })
        );
    }

    #[test]
    fn wait_for_event_options_match_the_runtime_shape() {
        let options = WorkflowWaitForEventOptions::new("approval").timeout("24 hours");

        assert_eq!(
            serde_json::to_value(options).unwrap(),
            json!({ "type": "approval", "timeout": "24 hours" })
        );
    }
}
