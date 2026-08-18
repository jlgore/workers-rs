use std::{
    env::{self, VarError},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result};
use clap::Parser;

const SHIM_FILE: &str = include_str!("./js/shim.js");
const SHIM_UNWIND_FILE: &str = include_str!("./js/shim-unwind.js");

pub(crate) mod binary;
mod build;
mod build_lock;
mod emoji;
mod lockfile;
mod main_legacy;
mod producers;
mod versions;

use build::{Build, BuildOptions};
use build_lock::BuildLock;

use crate::{
    binary::{Esbuild, GetBinary},
    build::Target,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn fix_wasm_import(out_dir: &Path) -> Result<()> {
    let index_path = output_path(out_dir, "index.js");
    let content = fs::read_to_string(&index_path)
        .with_context(|| format!("Failed to read {}", index_path.display()))?;
    let updated_content = content.replace("import source ", "import ");
    fs::write(&index_path, updated_content)
        .with_context(|| format!("Failed to write {}", index_path.display()))?;
    Ok(())
}

fn update_package_json(out_dir: &Path) -> Result<()> {
    let package_json_path = output_path(out_dir, "package.json");

    let original_content = fs::read_to_string(&package_json_path)
        .with_context(|| format!("Failed to read {}", package_json_path.display()))?;
    let mut package_json: serde_json::Value = serde_json::from_str(&original_content)?;

    package_json["files"] = serde_json::json!(["index_bg.wasm", "index.js", "index.d.ts"]);
    package_json["main"] = serde_json::Value::String("index.js".to_string());
    package_json["sideEffects"] = serde_json::json!(["./index.js"]);

    let updated_content = serde_json::to_string_pretty(&package_json)?;
    fs::write(&package_json_path, updated_content)
        .with_context(|| format!("Failed to write {}", package_json_path.display()))?;
    Ok(())
}

pub fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<_> = env::args().collect();
    if args.len() > 1 && (args[1].as_str() == "--version" || args[1].as_str() == "-v") {
        println!("{VERSION}");
        return Ok(());
    }
    let no_panic_recovery = args.iter().any(|a| a == "--no-panic-recovery");

    let wasm_pack_opts = parse_wasm_pack_opts(env::args().skip(1))?;
    let mut builder = Build::try_from_opts(wasm_pack_opts)?;

    // IMPORTANT: Build output is always relative to the crate root discovered by
    // `Build::try_from_opts`, not the process current working directory.
    let out_dir = builder.out_dir.clone();

    // Acquire the build lock: waits for any concurrent build to finish,
    // then creates a fresh .tmp staging directory with a heartbeat thread.
    let lock = BuildLock::acquire(&out_dir)?;
    let staging_dir = lock.staging_dir().to_path_buf();

    // Point the builder at the staging directory
    builder.out_dir = staging_dir.clone();

    builder.init()?;

    let module_target = !no_panic_recovery && env::var("CUSTOM_SHIM").is_err();
    if module_target {
        builder.extra_args.extend_from_slice(&[
            "--experimental-reset-state-function".into(),
            "--force-enable-abort-handler".into(),
        ]);
        builder.run()?;
    } else {
        builder.target = Target::Bundler;
        builder.run()?;
    }

    let with_coredump = env::var("COREDUMP").is_ok();
    if with_coredump {
        println!("Adding wasm coredump");
        wasm_coredump(&staging_dir)?;
    }

    producers::inject_workers_rs_sdk_metadata(&staging_dir, VERSION)?;

    if module_target {
        let shim = if builder.panic_unwind {
            SHIM_UNWIND_FILE
        } else {
            SHIM_FILE
        }
        .replace("$HANDLERS", &generate_handlers(&staging_dir)?);
        let shim_path = output_path(&staging_dir, "shim.js");
        fs::write(&shim_path, shim)
            .with_context(|| format!("Failed to write {}", shim_path.display()))?;

        add_export_wrappers(&staging_dir)?;

        update_package_json(&staging_dir)?;

        let esbuild_path = Esbuild.get_binary(None)?.0;
        bundle(&staging_dir, &esbuild_path)?;

        fix_wasm_import(&staging_dir)?;

        remove_unused_files(&staging_dir)?;

        create_wrapper_alias(&staging_dir, false)?;
    } else {
        main_legacy::process(&staging_dir)?;
        create_wrapper_alias(&staging_dir, true)?;
    }

    // Swap staging entries into the real output directory and clean up.
    lock.finish()?;

    Ok(())
}

const WORKFLOW_ENTRYPOINT_MARKER_PREFIX: &str = "__worker_workflow_entrypoint_";

#[derive(Debug, Default, PartialEq, Eq)]
struct WasmExports {
    functions: Vec<String>,
    classes: Vec<String>,
    workflow_entrypoints: Vec<String>,
}

fn is_valid_javascript_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('a'..='z' | 'A'..='Z' | '_' | '$'))
        && chars
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
}

fn discover_wasm_exports(content: &str) -> Result<WasmExports> {
    let mut exports = WasmExports::default();

    // Extract ESM exports from the wasm-bindgen generated output. This is specialized to what
    // wasm-bindgen currently emits and should eventually be replaced with Wasm export analysis.
    for line in content.lines() {
        let function_name = if let Some(rest) = line.strip_prefix("export function") {
            rest.find('(').map(|position| rest[..position].trim())
        } else if let Some(rest) = line.strip_prefix("export {") {
            rest.find(" as ").and_then(|position| {
                let alias = &rest[position + 4..];
                alias.find('}').map(|end| alias[..end].trim())
            })
        } else {
            None
        };

        if let Some(function_name) = function_name {
            if let Some(class_name) = function_name.strip_prefix(WORKFLOW_ENTRYPOINT_MARKER_PREFIX)
            {
                anyhow::ensure!(
                    is_valid_javascript_identifier(class_name),
                    "invalid Workflow entrypoint class name in build marker: {class_name}"
                );
                anyhow::ensure!(
                    !exports
                        .workflow_entrypoints
                        .iter()
                        .any(|name| name == class_name),
                    "duplicate Workflow entrypoint build marker for {class_name}"
                );
                exports.workflow_entrypoints.push(class_name.to_owned());
            } else if !SYSTEM_FNS.contains(&function_name) {
                exports.functions.push(function_name.to_owned());
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("export class ") {
            if let Some(brace_position) = rest.find('{') {
                exports
                    .classes
                    .push(rest[..brace_position].trim().to_owned());
            }
        }
    }

    Ok(exports)
}

fn generate_handlers(out_dir: &Path) -> Result<String> {
    let index_path = output_path(out_dir, "index.js");
    let content = fs::read_to_string(&index_path)
        .with_context(|| format!("Failed to read {}", index_path.display()))?;
    let exports = discover_wasm_exports(&content)?;

    let mut handlers = String::new();
    for func_name in exports.functions {
        if func_name == "fetch" && env::var("RUN_TO_COMPLETION").is_ok() {
            handlers += "Entrypoint.prototype.fetch = async function fetch(request) {
  let response = exports.fetch(request, this.env, this.ctx);
  this.ctx.waitUntil(response);
  return response;
}
";
        } else if func_name == "fetch"
            || func_name == "queue"
            || func_name == "scheduled"
            || func_name == "email"
        {
            // TODO: Switch these over to https://github.com/wasm-bindgen/wasm-bindgen/pull/4757
            // once that lands.
            handlers += &format!(
                "Entrypoint.prototype.{func_name} = function {func_name} (arg) {{
  return exports.{func_name}.call(this, arg, this.env, this.ctx);
}}
"
            );
        } else {
            handlers += &format!("Entrypoint.prototype.{func_name} = exports.{func_name};\n");
        }
    }

    Ok(handlers)
}

static SYSTEM_FNS: &[&str] = &["__wbg_reset_state", "__worker_init_state"];

fn render_export_wrappers(exports: &WasmExports) -> Result<String> {
    validate_workflow_entrypoints(exports)?;

    let mut wrappers = String::new();
    for class_name in &exports.classes {
        if exports
            .workflow_entrypoints
            .iter()
            .any(|entrypoint| entrypoint == class_name)
        {
            wrappers.push_str(&format!(
                "export const {class_name} = new Proxy(\n  class {class_name} extends WorkflowEntrypoint {{\n    constructor(ctx, env) {{\n      super(ctx, env);\n      this.inner = new exports.{class_name}(ctx, env);\n    }}\n\n    run(event, step) {{\n      return this.inner.run(event, step);\n    }}\n  }},\n  classProxyHooks,\n);\n"
            ));
        } else {
            wrappers.push_str(&format!(
                "export const {class_name} = new Proxy(exports.{class_name}, classProxyHooks);\n"
            ));
        }
    }

    Ok(wrappers)
}

fn validate_workflow_entrypoints(exports: &WasmExports) -> Result<()> {
    for workflow_entrypoint in &exports.workflow_entrypoints {
        anyhow::ensure!(
            exports
                .classes
                .iter()
                .any(|class_name| class_name == workflow_entrypoint),
            "Workflow entrypoint {workflow_entrypoint} does not have a matching class export"
        );
    }

    Ok(())
}

fn render_legacy_workflow_exports(exports: &WasmExports) -> Result<String> {
    validate_workflow_entrypoints(exports)?;

    let mut wrappers = String::new();
    for class_name in &exports.workflow_entrypoints {
        wrappers.push_str(&format!(
            "export class {class_name} extends WorkflowEntrypoint {{\n  constructor(ctx, env) {{\n    super(ctx, env);\n    this.inner = new imports.{class_name}(ctx, env);\n  }}\n\n  run(event, step) {{\n    return this.inner.run(event, step);\n  }}\n}}\n"
        ));
    }

    Ok(wrappers)
}

fn add_export_wrappers(out_dir: &Path) -> Result<()> {
    let index_path = output_path(out_dir, "index.js");
    let content = fs::read_to_string(&index_path)
        .with_context(|| format!("Failed to read {}", index_path.display()))?;

    let exports = discover_wasm_exports(&content)?;

    let shim_path = output_path(out_dir, "shim.js");
    let mut output = fs::read_to_string(&shim_path)
        .with_context(|| format!("Failed to read {}", shim_path.display()))?;
    let workflow_import = if exports.workflow_entrypoints.is_empty() {
        ""
    } else {
        "import { WorkflowEntrypoint } from \"cloudflare:workers\";"
    };
    output = output.replace("$WORKFLOW_IMPORT", workflow_import);
    output.push_str(&render_export_wrappers(&exports)?);
    fs::write(&shim_path, output)
        .with_context(|| format!("Failed to write {}", shim_path.display()))?;
    Ok(())
}

const INSTALL_HELP: &str = "In case you are missing the binary, you can install it using: `cargo install wasm-coredump-rewriter`";

fn wasm_coredump(out_dir: &Path) -> Result<()> {
    let coredump_flags = env::var("COREDUMP_FLAGS");
    let coredump_flags: Vec<&str> = if let Ok(flags) = &coredump_flags {
        flags.split(' ').collect()
    } else {
        vec![]
    };

    let mut child = Command::new("wasm-coredump-rewriter")
        .args(coredump_flags)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| {
            anyhow::anyhow!("failed to spawn wasm-coredump-rewriter: {err}\n\n{INSTALL_HELP}.")
        })?;

    let input_filename = output_path(out_dir, "index.wasm");

    let input_bytes = {
        let mut input = File::open(input_filename.clone())
            .map_err(|err| anyhow::anyhow!("failed to open input file: {err}"))?;

        let mut input_bytes = Vec::new();
        input
            .read_to_end(&mut input_bytes)
            .map_err(|err| anyhow::anyhow!("failed to open input file: {err}"))?;

        input_bytes
    };

    {
        let child_stdin = child.stdin.as_mut().unwrap();
        child_stdin
            .write_all(&input_bytes)
            .map_err(|err| anyhow::anyhow!("failed to write input file to rewriter: {err}"))?;
        // Close stdin to finish and avoid indefinite blocking
    }

    let output = child
        .wait_with_output()
        .map_err(|err| anyhow::anyhow!("failed to get rewriter's status: {err}"))?;

    if output.status.success() {
        // Open the input file again with truncate to write the output
        let mut f = fs::OpenOptions::new()
            .truncate(true)
            .write(true)
            .open(input_filename)
            .map_err(|err| anyhow::anyhow!("failed to open output file: {err}"))?;
        f.write_all(&output.stdout)
            .map_err(|err| anyhow::anyhow!("failed to write output file: {err}"))?;

        Ok(())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(format!(
            "failed to run Wasm coredump rewriter: {stdout}\n{stderr}"
        )))
    }
}

fn create_wrapper_alias(out_dir: &Path, legacy: bool) -> Result<()> {
    let msg = if !legacy {
        "// Use index.js directly, this file provided for backwards compat
// with former shim.mjs only.
"
    } else {
        ""
    };
    let path = if !legacy {
        "../index.js"
    } else {
        "./worker/shim.mjs"
    };
    let shim_content = format!(
        "{msg}export * from '{path}';
export {{ default }} from '{path}';
"
    );

    if !legacy {
        let worker_dir = output_path(out_dir, "worker");
        fs::create_dir_all(&worker_dir)
            .with_context(|| format!("Failed to create directory {}", worker_dir.display()))?;
        let shim_path = output_path(out_dir, "worker/shim.mjs");
        fs::write(&shim_path, shim_content)
            .with_context(|| format!("Failed to write {}", shim_path.display()))?;
    } else {
        let index_path = output_path(out_dir, "index.js");
        fs::write(&index_path, shim_content)
            .with_context(|| format!("Failed to write {}", index_path.display()))?;
    }
    Ok(())
}

#[derive(Parser)]
struct BuildArgs {
    #[clap(flatten)]
    pub build_options: BuildOptions,
}

fn parse_wasm_pack_opts<I>(args: I) -> Result<BuildOptions>
where
    I: IntoIterator<Item = String>,
{
    // This is done instead of explicitly constructing
    // BuildOptions to preserve the behavior of appending
    // arbitrary arguments in `args`.
    let mut build_args = vec![
        env!("CARGO_BIN_NAME").to_owned(),
        "--no-typescript".to_owned(),
        "--target".to_owned(),
        "module".to_owned(),
        "--out-name".to_owned(),
        "index".to_owned(),
    ];

    build_args.extend(args);

    let command = BuildArgs::try_parse_from(build_args)?;
    Ok(command.build_options)
}

// Bundles the snippets and worker-related code into a single file.
fn bundle(out_dir: &Path, esbuild_path: &Path) -> Result<()> {
    let no_minify = !matches!(env::var("NO_MINIFY"), Err(VarError::NotPresent));
    let path = out_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve output directory {}", out_dir.display()))?;
    let esbuild_path = esbuild_path
        .canonicalize()
        .with_context(|| format!("Failed to resolve esbuild path {}", esbuild_path.display()))?;
    let mut command = Command::new(esbuild_path);
    command.args([
        "--external:./index_bg.wasm",
        "--external:cloudflare:email",
        "--external:cloudflare:sockets",
        "--external:cloudflare:workers",
        "--external:cloudflare:workflows",
        "--format=esm",
        "--bundle",
        "./shim.js",
        "--outfile=index.js",
        "--allow-overwrite",
    ]);

    if !no_minify {
        command.arg("--minify");
    }

    let exit_status = command.current_dir(path).spawn()?.wait()?;

    match exit_status.success() {
        true => Ok(()),
        false => anyhow::bail!("esbuild exited with status {exit_status}"),
    }
}

fn remove_unused_files(out_dir: &Path) -> Result<()> {
    let shim_path = output_path(out_dir, "shim.js");
    std::fs::remove_file(&shim_path)
        .with_context(|| format!("Failed to remove {}", shim_path.display()))?;
    let snippets_path = output_path(out_dir, "snippets");
    if snippets_path.exists() {
        std::fs::remove_dir_all(&snippets_path)
            .with_context(|| format!("Failed to remove {}", snippets_path.display()))?;
    }
    Ok(())
}

pub fn output_path(out_dir: &Path, name: impl AsRef<str>) -> PathBuf {
    out_dir.join(name.as_ref())
}

#[cfg(test)]
mod test {
    use super::{
        discover_wasm_exports, parse_wasm_pack_opts, render_export_wrappers,
        render_legacy_workflow_exports,
    };
    #[test]
    fn test_wasm_pack_args_build_arg() {
        let args = vec!["--release".to_owned()];
        let result = parse_wasm_pack_opts(args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_out_dir_default() {
        let args: Vec<String> = vec![];
        let result = parse_wasm_pack_opts(args).unwrap();
        assert_eq!(result.out_dir, "build");
    }

    #[test]
    fn test_out_dir_override() {
        let args = vec!["--out-dir".to_owned(), "dist/worker".to_owned()];
        let result = parse_wasm_pack_opts(args).unwrap();
        assert_eq!(result.out_dir, "dist/worker");
    }

    #[test]
    fn discovers_workflow_entrypoint_markers_without_treating_them_as_handlers() {
        let source = r#"
export function fetch(arg0, arg1, arg2) {}
export function __worker_workflow_entrypoint_OrderWorkflow() {}
export class Counter {}
export class OrderWorkflow {}
"#;

        let exports = discover_wasm_exports(source).unwrap();

        assert_eq!(exports.functions, vec!["fetch"]);
        assert_eq!(exports.classes, vec!["Counter", "OrderWorkflow"]);
        assert_eq!(exports.workflow_entrypoints, vec!["OrderWorkflow"]);
    }

    #[test]
    fn renders_real_workflow_subclasses_and_keeps_normal_class_proxies() {
        let source = r#"
export function __worker_workflow_entrypoint_OrderWorkflow() {}
export class Counter {}
export class OrderWorkflow {}
"#;
        let exports = discover_wasm_exports(source).unwrap();

        let wrappers = render_export_wrappers(&exports).unwrap();

        assert!(wrappers.contains("class OrderWorkflow extends WorkflowEntrypoint"));
        assert!(wrappers.contains("this.inner = new exports.OrderWorkflow(ctx, env);"));
        assert!(wrappers
            .contains("export const Counter = new Proxy(exports.Counter, classProxyHooks);"));
        assert!(!wrappers.contains("new Proxy(exports.OrderWorkflow, classProxyHooks)"));
    }

    #[test]
    fn rejects_a_workflow_marker_without_a_matching_class_export() {
        let source = "export function __worker_workflow_entrypoint_MissingWorkflow() {}";
        let exports = discover_wasm_exports(source).unwrap();

        let error = render_export_wrappers(&exports).unwrap_err();

        assert!(error
            .to_string()
            .contains("MissingWorkflow does not have a matching class export"));
    }

    #[test]
    fn renders_workflow_subclasses_for_the_legacy_shim() {
        let source = r#"
export function __worker_workflow_entrypoint_OrderWorkflow() {}
export class OrderWorkflow {}
"#;
        let exports = discover_wasm_exports(source).unwrap();

        let wrappers = render_legacy_workflow_exports(&exports).unwrap();

        assert!(wrappers.contains("export class OrderWorkflow extends WorkflowEntrypoint"));
        assert!(wrappers.contains("this.inner = new imports.OrderWorkflow(ctx, env);"));
    }
}
