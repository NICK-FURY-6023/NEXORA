use nexora_error::ErrorCode;
use nexora_plugin_api::{PluginCapability, PluginManifest, PluginRuntime, SandboxPolicy};

fn manifest() -> PluginManifest {
    PluginManifest::new("test-plugin", "0.1.0", "plugin.wasm")
}

/// A module that exports `answer`, which returns the i32 constant 42.
const ANSWER_MODULE: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // type: () -> i32
    0x03, 0x02, 0x01, 0x00, // function: one function of type 0
    0x07, 0x0a, 0x01, 0x06, b'a', b'n', b's', b'w', b'e', b'r', 0x00, 0x00, // export
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b, // code: i32.const 42, end
];

#[test]
fn a_module_runs_its_exported_function() {
    let runtime = PluginRuntime::new().expect("runtime should start");
    let result = runtime
        .call(&manifest(), ANSWER_MODULE, "answer", &[])
        .expect("the module should run");
    assert_eq!(result, vec![42]);
}

#[test]
fn a_module_that_is_not_webassembly_is_refused() {
    let runtime = PluginRuntime::new().expect("runtime should start");
    let error = runtime
        .call(&manifest(), b"not a wasm module", "answer", &[])
        .expect_err("a file that is not webassembly must be refused");
    assert_eq!(error.code(), ErrorCode::InvalidInput);
}

#[test]
fn a_module_without_the_named_export_is_refused() {
    let runtime = PluginRuntime::new().expect("runtime should start");
    let error = runtime
        .call(&manifest(), ANSWER_MODULE, "missing", &[])
        .expect_err("an unknown export must be refused");
    assert_eq!(error.code(), ErrorCode::InvalidInput);
}

#[test]
fn a_module_that_never_returns_is_stopped_by_the_time_limit() {
    let mut policy = SandboxPolicy::deny_all();
    policy.time_limit_ms = 50;
    let runtime = PluginRuntime::with_policy(policy).expect("runtime should start");
    // An infinite loop: the fuel the time limit buys must run out.
    let error = runtime
        .call(&manifest(), INFINITE_LOOP_MODULE, "loop", &[])
        .expect_err("the time limit must stop the module");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_module_that_asks_for_a_denied_host_is_refused() {
    let mut policy = SandboxPolicy::deny_all();
    policy.network = true;
    policy.allowed_hosts = vec!["api.example.com".to_owned()];
    policy.time_limit_ms = 1_000;
    let runtime = PluginRuntime::with_policy(policy).expect("runtime should start");
    let error = runtime
        .call(
            &manifest(),
            FETCH_MODULE,
            "fetch",
            &["https://evil.example.org/"],
        )
        .expect_err("a denied host must be refused");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_module_that_asks_for_an_allowed_host_is_allowed() {
    let mut policy = SandboxPolicy::deny_all();
    policy.network = true;
    policy.allowed_hosts = vec!["api.example.com".to_owned()];
    policy.time_limit_ms = 1_000;
    let runtime = PluginRuntime::with_policy(policy).expect("runtime should start");
    runtime
        .call(
            &manifest(),
            FETCH_MODULE,
            "fetch",
            &["https://api.example.com/track"],
        )
        .expect("an allowed host must be allowed");
}

/// A module that calls the host's `fetch` function with the url it is given.
const FETCH_MODULE: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x06, 0x01, 0x60, 0x01, 0x7f, 0x01, 0x7f, // type: (i32) -> i32
    0x02, 0x0e, 0x01, // import: host.fetch
    0x04, b'h', b'o', b's', b't', 0x05, b'f', b'e', b't', b'c', b'h', 0x00, 0x00, 0x03, 0x02, 0x01,
    0x00, // function: one function of type 0
    0x07, 0x09, 0x01, 0x05, b'f', b'e', b't', b'c', b'h', 0x00, 0x00, // export
    0x0a, 0x08, 0x01, 0x06, 0x00, 0x20, 0x00, 0x10, 0x00, 0x0b, // code
];

/// A module whose exported function loops forever.
const INFINITE_LOOP_MODULE: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type: () -> ()
    0x03, 0x02, 0x01, 0x00, // function: one function of type 0
    0x07, 0x08, 0x01, 0x04, b'l', b'o', b'o', b'p', 0x00, 0x00, // export
    0x0a, 0x09, 0x01, 0x07, 0x00, 0x03, 0x40, 0x0c, 0x00, 0x0b, 0x0b, // code: loop
];

#[test]
fn the_manifest_capabilities_reach_the_host() {
    let manifest = PluginManifest::new("test-plugin", "0.1.0", "plugin.wasm")
        .with_capability(PluginCapability::Network("api.example.com".to_owned()));
    let mut policy = SandboxPolicy::deny_all();
    policy.network = true;
    policy.allowed_hosts = vec!["api.example.com".to_owned()];
    policy.time_limit_ms = 1_000;
    let runtime = PluginRuntime::with_policy(policy).expect("runtime should start");
    runtime
        .call(
            &manifest,
            FETCH_MODULE,
            "fetch",
            &["https://api.example.com/track"],
        )
        .expect("an allowed host must be allowed");
}
