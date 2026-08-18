//! Bindings for [Cloudflare Artifacts](https://developers.cloudflare.com/artifacts/).

use js_sys::futures::JsFuture;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use worker_sys::{Artifacts as ArtifactsSys, ArtifactsRepo as ArtifactsRepoSys};

use crate::{EnvBinding, Result};

/// A Cloudflare Artifacts namespace binding.
#[derive(Debug, Clone)]
pub struct Artifacts(ArtifactsSys);

unsafe impl Send for Artifacts {}
unsafe impl Sync for Artifacts {}

impl EnvBinding for Artifacts {
    const TYPE_NAME: &'static str = "Artifacts";

    // Artifacts is an interface backed by a service binding. Its concrete
    // constructor is an implementation detail and differs in local dev.
    fn get(val: JsValue) -> Result<Self> {
        Ok(val.unchecked_into())
    }
}

impl JsCast for Artifacts {
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

impl AsRef<JsValue> for Artifacts {
    fn as_ref(&self) -> &JsValue {
        &self.0
    }
}

impl From<Artifacts> for JsValue {
    fn from(artifacts: Artifacts) -> Self {
        artifacts.0.into()
    }
}

impl From<ArtifactsSys> for Artifacts {
    fn from(artifacts: ArtifactsSys) -> Self {
        Self(artifacts)
    }
}

impl Artifacts {
    /// Create a repository using the service defaults.
    pub async fn create(&self, name: impl AsRef<str>) -> Result<ArtifactsCreateRepoResult> {
        self.create_inner(name.as_ref(), &JsValue::undefined())
            .await
    }

    /// Create a repository with explicit options.
    pub async fn create_with_options(
        &self,
        name: impl AsRef<str>,
        options: &ArtifactsCreateOptions,
    ) -> Result<ArtifactsCreateRepoResult> {
        let options = serde_wasm_bindgen::to_value(options)?;
        self.create_inner(name.as_ref(), &options).await
    }

    async fn create_inner(
        &self,
        name: &str,
        options: &JsValue,
    ) -> Result<ArtifactsCreateRepoResult> {
        let value = JsFuture::from(self.0.create(name, options)?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    /// Get a handle to an existing repository.
    pub async fn get(&self, name: impl AsRef<str>) -> Result<ArtifactsRepo> {
        let value = JsFuture::from(self.0.get(name.as_ref())?).await?;
        Ok(ArtifactsRepo(value.unchecked_into()))
    }

    /// Import a repository from an external HTTPS Git remote.
    pub async fn import(
        &self,
        params: &ArtifactsImportParams,
    ) -> Result<ArtifactsCreateRepoResult> {
        let params = serde_wasm_bindgen::to_value(params)?;
        let value = JsFuture::from(self.0.import(&params)?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    /// List repositories using the service defaults.
    pub async fn list(&self) -> Result<ArtifactsRepoListResult> {
        self.list_inner(&JsValue::undefined()).await
    }

    /// List repositories with pagination options.
    pub async fn list_with_options(
        &self,
        options: &ArtifactsListOptions,
    ) -> Result<ArtifactsRepoListResult> {
        let options = serde_wasm_bindgen::to_value(options)?;
        self.list_inner(&options).await
    }

    async fn list_inner(&self, options: &JsValue) -> Result<ArtifactsRepoListResult> {
        let value = JsFuture::from(self.0.list(options)?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    /// Delete a repository and its associated tokens.
    ///
    /// Returns `false` when the repository was not found.
    pub async fn delete(&self, name: impl AsRef<str>) -> Result<bool> {
        let value = JsFuture::from(self.0.delete(name.as_ref())?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }
}

/// Options used when creating an Artifacts repository.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsCreateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_default_branch: Option<String>,
}

impl ArtifactsCreateOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = Some(read_only);
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn default_branch(mut self, branch: impl Into<String>) -> Self {
        self.set_default_branch = Some(branch.into());
        self
    }
}

/// Parameters for importing an external Git repository into Artifacts.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactsImportParams {
    pub source: ArtifactsImportSource,
    pub target: ArtifactsImportTarget,
}

impl ArtifactsImportParams {
    pub fn new(url: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            source: ArtifactsImportSource::new(url),
            target: ArtifactsImportTarget::new(name),
        }
    }
}

/// Source settings for an Artifacts repository import.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactsImportSource {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

impl ArtifactsImportSource {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            branch: None,
            depth: None,
        }
    }

    #[must_use]
    pub fn branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    #[must_use]
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = Some(depth);
        self
    }
}

/// Target settings for an Artifacts repository import.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactsImportTarget {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opts: Option<ArtifactsImportTargetOptions>,
}

impl ArtifactsImportTarget {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            opts: None,
        }
    }

    #[must_use]
    pub fn options(mut self, options: ArtifactsImportTargetOptions) -> Self {
        self.opts = Some(options);
        self
    }
}

/// Options applied to the target of an Artifacts repository import.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsImportTargetOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

impl ArtifactsImportTargetOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = Some(read_only);
        self
    }
}

/// Pagination options for listing Artifacts repositories.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArtifactsListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

impl ArtifactsListOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }
}

/// Metadata and the initial token returned when a repository is created,
/// imported, or forked.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsCreateRepoResult {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub remote: String,
    pub token: String,
    pub token_expires_at: String,
}

/// Metadata for an Artifacts repository.
///
/// `remote` is absent from entries returned by [`Artifacts::list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsRepoInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_push_at: Option<String>,
    pub source: Option<String>,
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ArtifactsRepoStatus>,
}

/// State of an asynchronous Artifacts repository operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactsRepoStatus {
    Ready,
    Importing,
    Forking,
    #[serde(other)]
    Unknown,
}

/// A page of Artifacts repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactsRepoListResult {
    pub repos: Vec<ArtifactsRepoInfo>,
    pub total: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Scope assigned to an Artifacts repository token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactsTokenScope {
    Read,
    Write,
}

/// State of an Artifacts repository token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactsTokenState {
    Active,
    Expired,
    Revoked,
    #[serde(other)]
    Unknown,
}

/// A newly-created Artifacts repository token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsCreateTokenResult {
    pub id: String,
    pub plaintext: String,
    pub scope: ArtifactsTokenScope,
    pub expires_at: String,
}

/// Metadata for an Artifacts repository token. The plaintext token is not
/// included.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsTokenInfo {
    pub id: String,
    pub scope: ArtifactsTokenScope,
    pub state: ArtifactsTokenState,
    pub created_at: String,
    pub expires_at: String,
}

/// Tokens belonging to an Artifacts repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactsTokenListResult {
    pub tokens: Vec<ArtifactsTokenInfo>,
    pub total: u32,
}

/// A handle to one Artifacts repository.
#[derive(Debug, Clone)]
pub struct ArtifactsRepo(ArtifactsRepoSys);

unsafe impl Send for ArtifactsRepo {}
unsafe impl Sync for ArtifactsRepo {}

impl JsCast for ArtifactsRepo {
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

impl AsRef<JsValue> for ArtifactsRepo {
    fn as_ref(&self) -> &JsValue {
        &self.0
    }
}

impl From<ArtifactsRepo> for JsValue {
    fn from(repo: ArtifactsRepo) -> Self {
        repo.0.into()
    }
}

impl From<ArtifactsRepoSys> for ArtifactsRepo {
    fn from(repo: ArtifactsRepoSys) -> Self {
        Self(repo)
    }
}

impl ArtifactsRepo {
    pub fn id(&self) -> String {
        self.0.id()
    }

    pub fn name(&self) -> String {
        self.0.name()
    }

    pub fn description(&self) -> Option<String> {
        self.0.description()
    }

    pub fn default_branch(&self) -> String {
        self.0.default_branch()
    }

    pub fn created_at(&self) -> String {
        self.0.created_at()
    }

    pub fn updated_at(&self) -> String {
        self.0.updated_at()
    }

    pub fn last_push_at(&self) -> Option<String> {
        self.0.last_push_at()
    }

    pub fn source(&self) -> Option<String> {
        self.0.source()
    }

    pub fn read_only(&self) -> bool {
        self.0.read_only()
    }

    pub fn remote(&self) -> String {
        self.0.remote()
    }

    /// Create a write-scoped token using the service's default TTL.
    pub async fn create_token(&self) -> Result<ArtifactsCreateTokenResult> {
        self.create_token_inner(&JsValue::undefined(), &JsValue::undefined())
            .await
    }

    /// Create a token with an explicit scope and optional TTL in seconds.
    pub async fn create_token_with_options(
        &self,
        scope: ArtifactsTokenScope,
        ttl: Option<u32>,
    ) -> Result<ArtifactsCreateTokenResult> {
        let scope = serde_wasm_bindgen::to_value(&scope)?;
        let ttl = serde_wasm_bindgen::to_value(&ttl)?;
        self.create_token_inner(&scope, &ttl).await
    }

    async fn create_token_inner(
        &self,
        scope: &JsValue,
        ttl: &JsValue,
    ) -> Result<ArtifactsCreateTokenResult> {
        let value = JsFuture::from(self.0.create_token(scope, ttl)?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    pub async fn list_tokens(&self) -> Result<ArtifactsTokenListResult> {
        let value = JsFuture::from(self.0.list_tokens()?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    /// Revoke a token by its plaintext value or ID.
    ///
    /// Returns `false` if the token was not found.
    pub async fn revoke_token(&self, token_or_id: impl AsRef<str>) -> Result<bool> {
        let value = JsFuture::from(self.0.revoke_token(token_or_id.as_ref())?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    /// Fork this repository using the service defaults.
    pub async fn fork(&self, name: impl AsRef<str>) -> Result<ArtifactsCreateRepoResult> {
        self.fork_inner(name.as_ref(), &JsValue::undefined()).await
    }

    /// Fork this repository with explicit options.
    pub async fn fork_with_options(
        &self,
        name: impl AsRef<str>,
        options: &ArtifactsForkOptions,
    ) -> Result<ArtifactsCreateRepoResult> {
        let options = serde_wasm_bindgen::to_value(options)?;
        self.fork_inner(name.as_ref(), &options).await
    }

    async fn fork_inner(&self, name: &str, options: &JsValue) -> Result<ArtifactsCreateRepoResult> {
        let value = JsFuture::from(self.0.fork(name, options)?).await?;
        Ok(serde_wasm_bindgen::from_value(value)?)
    }

    /// Read commit history using the service defaults.
    ///
    /// The Artifacts beta does not yet publish this result's runtime type, so
    /// the value is returned without lossy conversion.
    pub async fn log(&self) -> Result<JsValue> {
        self.log_inner(&JsValue::undefined()).await
    }

    /// Read commit history with explicit options.
    pub async fn log_with_options(&self, options: &ArtifactsLogOptions) -> Result<JsValue> {
        let options = serde_wasm_bindgen::to_value(options)?;
        self.log_inner(&options).await
    }

    async fn log_inner(&self, options: &JsValue) -> Result<JsValue> {
        Ok(JsFuture::from(self.0.log(options)?).await?)
    }

    /// Read a commit by SHA-1 hash.
    ///
    /// The result remains a raw JavaScript value until Cloudflare publishes a
    /// stable runtime type for `ArtifactsCommit`.
    pub async fn read_commit(&self, hash: impl AsRef<str>) -> Result<JsValue> {
        Ok(JsFuture::from(self.0.read_commit(hash.as_ref())?).await?)
    }

    /// Read a tree by SHA-1 hash.
    ///
    /// The result remains a raw JavaScript value until Cloudflare publishes a
    /// stable runtime type for `ArtifactsTree`.
    pub async fn read_tree(&self, hash: impl AsRef<str>) -> Result<JsValue> {
        Ok(JsFuture::from(self.0.read_tree(hash.as_ref())?).await?)
    }
}

/// Options used when forking an Artifacts repository.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactsForkOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch_only: Option<bool>,
}

impl ArtifactsForkOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = Some(read_only);
        self
    }

    #[must_use]
    pub fn default_branch_only(mut self, default_branch_only: bool) -> Self {
        self.default_branch_only = Some(default_branch_only);
        self
    }
}

/// Options used when reading an Artifacts repository's commit history.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArtifactsLogOptions {
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

impl ArtifactsLogOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn git_ref(mut self, git_ref: impl Into<String>) -> Self {
        self.git_ref = Some(git_ref.into());
        self
    }

    #[must_use]
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn artifact_options_use_runtime_field_names() {
        let create = ArtifactsCreateOptions::new()
            .read_only(true)
            .description("test")
            .default_branch("trunk");
        assert_eq!(
            serde_json::to_value(create).unwrap(),
            json!({
                "readOnly": true,
                "description": "test",
                "setDefaultBranch": "trunk"
            })
        );

        let log = ArtifactsLogOptions::new()
            .git_ref("main")
            .limit(10)
            .offset(2);
        assert_eq!(
            serde_json::to_value(log).unwrap(),
            json!({ "ref": "main", "limit": 10, "offset": 2 })
        );
    }

    #[test]
    fn import_params_match_the_nested_runtime_shape() {
        let params = ArtifactsImportParams {
            source: ArtifactsImportSource::new("https://example.com/repo.git")
                .branch("main")
                .depth(1),
            target: ArtifactsImportTarget::new("copy").options(
                ArtifactsImportTargetOptions::new()
                    .description("imported")
                    .read_only(true),
            ),
        };

        assert_eq!(
            serde_json::to_value(params).unwrap(),
            json!({
                "source": {
                    "url": "https://example.com/repo.git",
                    "branch": "main",
                    "depth": 1
                },
                "target": {
                    "name": "copy",
                    "opts": {
                        "description": "imported",
                        "readOnly": true
                    }
                }
            })
        );
    }
}
