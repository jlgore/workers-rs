use js_sys::Promise;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub type Artifacts;

    #[wasm_bindgen(method, catch, js_name = create)]
    pub fn create(this: &Artifacts, name: &str, options: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = get)]
    pub fn get(this: &Artifacts, name: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = import)]
    pub fn import(this: &Artifacts, params: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = list)]
    pub fn list(this: &Artifacts, options: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = delete)]
    pub fn delete(this: &Artifacts, name: &str) -> Result<Promise, JsValue>;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub type ArtifactsRepo;

    #[wasm_bindgen(method, getter)]
    pub fn id(this: &ArtifactsRepo) -> String;

    #[wasm_bindgen(method, getter)]
    pub fn name(this: &ArtifactsRepo) -> String;

    #[wasm_bindgen(method, getter)]
    pub fn description(this: &ArtifactsRepo) -> Option<String>;

    #[wasm_bindgen(method, getter, js_name = defaultBranch)]
    pub fn default_branch(this: &ArtifactsRepo) -> String;

    #[wasm_bindgen(method, getter, js_name = createdAt)]
    pub fn created_at(this: &ArtifactsRepo) -> String;

    #[wasm_bindgen(method, getter, js_name = updatedAt)]
    pub fn updated_at(this: &ArtifactsRepo) -> String;

    #[wasm_bindgen(method, getter, js_name = lastPushAt)]
    pub fn last_push_at(this: &ArtifactsRepo) -> Option<String>;

    #[wasm_bindgen(method, getter)]
    pub fn source(this: &ArtifactsRepo) -> Option<String>;

    #[wasm_bindgen(method, getter, js_name = readOnly)]
    pub fn read_only(this: &ArtifactsRepo) -> bool;

    #[wasm_bindgen(method, getter)]
    pub fn remote(this: &ArtifactsRepo) -> String;

    #[wasm_bindgen(method, catch, js_name = createToken)]
    pub fn create_token(
        this: &ArtifactsRepo,
        scope: &JsValue,
        ttl: &JsValue,
    ) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = listTokens)]
    pub fn list_tokens(this: &ArtifactsRepo) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = revokeToken)]
    pub fn revoke_token(this: &ArtifactsRepo, token_or_id: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = fork)]
    pub fn fork(this: &ArtifactsRepo, name: &str, options: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = log)]
    pub fn log(this: &ArtifactsRepo, options: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = readCommit)]
    pub fn read_commit(this: &ArtifactsRepo, hash: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = readTree)]
    pub fn read_tree(this: &ArtifactsRepo, hash: &str) -> Result<Promise, JsValue>;
}
