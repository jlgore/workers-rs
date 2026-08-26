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

    // The handle is a JsRpcStub: it has no data properties, so accessors
    // declared here would return an RPC proxy rather than a value, and awaiting
    // one is rejected outright ("the RPC receiver does not implement the method
    // `remote`"). Metadata comes from info(), which answers with plain data.
    #[wasm_bindgen(method, catch, js_name = info)]
    pub fn info(this: &ArtifactsRepo) -> Result<Promise, JsValue>;

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
