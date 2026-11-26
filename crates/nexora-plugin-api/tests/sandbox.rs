use std::fs;
use std::time::Duration;

use nexora_error::ErrorCode;
use nexora_plugin_api::{PluginCapability, PluginManifest, PluginSandbox, SandboxPolicy};

fn manifest_with(capabilities: Vec<PluginCapability>) -> PluginManifest {
    let mut manifest = PluginManifest::new("demo", "0.1.0", "plugin.wasm");
    for capability in capabilities {
        manifest = manifest.with_capability(capability);
    }
    manifest
}

fn network_sandbox() -> PluginSandbox {
    PluginSandbox::from_manifest(&manifest_with(vec![PluginCapability::Network(
        "api.example.com".to_owned(),
    )]))
    .expect("manifest should be valid")
}

#[test]
fn network_is_denied_without_the_capability() {
    let sandbox = PluginSandbox::from_manifest(&manifest_with(vec![])).expect("valid manifest");
    let error = sandbox
        .check_network("https://api.example.com/track")
        .expect_err("network must be denied");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn network_is_allowed_for_a_listed_host() {
    network_sandbox()
        .check_network("https://api.example.com/track")
        .expect("listed host is allowed");
}

#[test]
fn network_is_denied_for_an_unlisted_host() {
    let error = network_sandbox()
        .check_network("https://evil.example.org/track")
        .expect_err("unlisted host must be denied");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn network_is_denied_for_a_private_address_even_when_listed() {
    let sandbox = PluginSandbox::from_manifest(&manifest_with(vec![PluginCapability::Network(
        "127.0.0.1".to_owned(),
    )]))
    .expect("valid manifest");
    let error = sandbox
        .check_network("https://127.0.0.1/track")
        .expect_err("loopback must be denied");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn network_rejects_a_non_http_scheme() {
    let error = network_sandbox()
        .check_network("file:///etc/passwd")
        .expect_err("file scheme must be denied");
    assert_eq!(error.code(), ErrorCode::InvalidInput);
}

fn filesystem_sandbox() -> PluginSandbox {
    PluginSandbox::from_manifest(&manifest_with(vec![PluginCapability::FileSystem(
        "data".to_owned(),
    )]))
    .expect("manifest should be valid")
}

#[test]
fn filesystem_is_denied_without_the_capability() {
    let sandbox = PluginSandbox::from_manifest(&manifest_with(vec![])).expect("valid manifest");
    let error = sandbox
        .check_path("data/track.bin")
        .expect_err("filesystem must be denied");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_path_inside_the_allowed_root_is_allowed() {
    filesystem_sandbox()
        .check_path("data/track.bin")
        .expect("listed path is allowed");
}

#[test]
fn a_path_outside_the_allowed_root_is_denied() {
    let error = filesystem_sandbox()
        .check_path("secrets/token.txt")
        .expect_err("unlisted path must be denied");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_traversing_path_is_denied() {
    let error = filesystem_sandbox()
        .check_path("data/../../etc/passwd")
        .expect_err("traversal must be denied");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn an_absolute_path_is_denied() {
    let error = filesystem_sandbox()
        .check_path("/etc/passwd")
        .expect_err("absolute path must be denied");
    assert_eq!(error.code(), ErrorCode::InvalidInput);
}

#[test]
fn allocation_beyond_the_memory_limit_is_denied() {
    let mut sandbox = PluginSandbox::from_manifest(&manifest_with(vec![])).expect("valid manifest");
    sandbox.set_memory_limit_mb(1);
    sandbox.allocate(512 * 1024).expect("first allocation fits");
    let error = sandbox
        .allocate(1024 * 1024)
        .expect_err("second allocation exceeds the limit");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_run_past_the_time_limit_is_denied() {
    let mut sandbox = PluginSandbox::from_manifest(&manifest_with(vec![])).expect("valid manifest");
    sandbox.set_time_limit_ms(0);
    std::thread::sleep(Duration::from_millis(5));
    let error = sandbox.check_time().expect_err("the time limit is spent");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_run_inside_the_time_limit_is_allowed() {
    let sandbox = PluginSandbox::from_manifest(&manifest_with(vec![])).expect("valid manifest");
    sandbox.check_time().expect("the time limit is not spent");
}

#[test]
fn the_granted_capability_set_comes_from_the_manifest() {
    let sandbox = network_sandbox();
    assert!(sandbox.grants(&PluginCapability::Network("api.example.com".to_owned())));
    assert!(!sandbox.grants(&PluginCapability::Network("evil.example.org".to_owned())));
    assert!(!sandbox.grants(&PluginCapability::AudioDecode));
}

#[test]
fn loading_a_module_resolves_the_entrypoint_inside_the_root() {
    let root = std::env::temp_dir().join(format!("nexora-plugin-load-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temp root should exist");
    fs::write(root.join("plugin.wasm"), b"\0asm").expect("module should be written");

    let manifest = manifest_with(vec![PluginCapability::FileSystem("data".to_owned())]);
    let module = PluginSandbox::from_manifest(&manifest)
        .expect("manifest should be valid")
        .load(&root, &manifest)
        .expect("module should load");
    assert_eq!(module.entrypoint(), root.join("plugin.wasm"));

    fs::remove_dir_all(&root).ok();
}

#[test]
fn loading_a_module_rejects_a_traversing_entrypoint() {
    let root = std::env::temp_dir().join(format!("nexora-plugin-traversal-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temp root should exist");

    let manifest = manifest_with(vec![]);
    let mut escaping = manifest.clone();
    escaping.entrypoint = "../outside.wasm".to_owned();
    let error = PluginSandbox::from_manifest(&manifest)
        .expect("manifest should be valid")
        .load(&root, &escaping)
        .expect_err("traversal must be rejected");
    assert_eq!(error.code(), ErrorCode::InvalidInput);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_policy_defaults_deny_everything() {
    let policy = SandboxPolicy::deny_all();
    assert!(!policy.network);
    assert!(!policy.filesystem);
    assert_eq!(policy.time_limit_ms, 0);
    assert_eq!(policy.memory_limit_mb, 0);
}
