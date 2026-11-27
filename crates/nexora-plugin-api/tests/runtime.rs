use nexora_error::ErrorCode;
use nexora_plugin_api::{
    MAX_PUSHED_STRINGS, PluginCapability, PluginManifest, PluginRuntime, SandboxPolicy,
};

fn manifest() -> PluginManifest {
    PluginManifest::new("test-plugin", "0.1.0", "plugin.wasm")
}

/// A module that exports `greet`, which hands the string "hello" back to the
/// host through `host.push` and returns the index it was given.
const GREET_MODULE: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x0b, 0x02, // type section: two types
    0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // (i32, i32) -> i32
    0x60, 0x00, 0x01, 0x7f, // () -> i32
    0x02, 0x0d, 0x01, // import section: host.push
    0x04, b'h', b'o', b's', b't', 0x04, b'p', b'u', b's', b'h', 0x00, 0x00, 0x03, 0x02, 0x01,
    0x01, // function section: one function of type 1
    0x05, 0x03, 0x01, 0x00, 0x01, // memory section: one page
    0x07, 0x12, 0x02, // export section: memory and greet
    0x06, b'm', b'e', b'm', b'o', b'r', b'y', 0x02, 0x00, 0x05, b'g', b'r', b'e', b'e', b't', 0x00,
    0x01, 0x0a, 0x0a, 0x01, 0x08, 0x00, 0x41, 0x00, 0x41, 0x05, 0x10, 0x00, 0x0b, // code
    0x0b, 0x0b, 0x01, 0x00, 0x41, 0x00, 0x0b, 0x05, b'h', b'e', b'l', b'l', b'o', // data
];

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
    assert_eq!(result.results, vec![42]);
}

#[test]
fn a_module_hands_a_string_back_to_the_host() {
    let runtime = PluginRuntime::new().expect("runtime should start");
    let result = runtime
        .call(&manifest(), GREET_MODULE, "greet", &[])
        .expect("the module should run");
    assert_eq!(result.strings, vec!["hello".to_owned()]);
    assert_eq!(result.results, vec![1]);
}

/// A module that pushes forty strings through `host.push` and returns the
/// index of the last one.
const PUSH_MANY_MODULE: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x0b, 0x02, // type section: (i32, i32) -> i32 and () -> i32
    0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x60, 0x00, 0x01, 0x7f, 0x02, 0x0d,
    0x01, // import section: host.push
    0x04, b'h', b'o', b's', b't', 0x04, b'p', b'u', b's', b'h', 0x00, 0x00, 0x03, 0x02, 0x01,
    0x01, // function section: one function of type 1
    0x05, 0x03, 0x01, 0x00, 0x01, // memory section: one page
    0x07, 0x15, 0x02, // export section: memory and pushmany
    0x06, b'm', b'e', b'm', b'o', b'r', b'y', 0x02, 0x00, 0x08, b'p', b'u', b's', b'h', b'm', b'a',
    b'n', b'y', 0x00, 0x01, 0x0a, 0x21, 0x01, 0x1f, 0x01, 0x02, 0x7f, // code: two i32 locals
    0x03, 0x40, 0x41, 0x00, 0x41, 0x04, 0x10, 0x00, 0x21, 0x01, 0x20, 0x00, 0x41, 0x01, 0x6a, 0x21,
    0x00, 0x20, 0x00, 0x41, 0x28, 0x48, 0x0d, 0x00, 0x0b, 0x20, 0x01, 0x0b, 0x0b, 0x0a, 0x01, 0x00,
    0x41, 0x00, 0x0b, 0x04, b'a', b'a', b'a', b'a', // data
];
#[test]
fn a_module_cannot_make_the_host_hold_an_unbounded_number_of_strings() {
    // The sandbox bounds the module's own memory, but the strings it pushes
    // live in the host. A module that pushes without end must be stopped at
    // the point the host's own limit is reached.
    let mut policy = SandboxPolicy::deny_all();
    policy.time_limit_ms = 5_000;
    policy.memory_limit_mb = 1;
    let runtime = PluginRuntime::with_policy(policy).expect("runtime should start");
    let result = runtime.call(&manifest(), PUSH_MANY_MODULE, "pushmany", &[]);
    match result {
        Ok(call) => assert!(
            call.strings.len() <= MAX_PUSHED_STRINGS,
            "the host held {} strings",
            call.strings.len()
        ),
        Err(error) => assert_eq!(error.code(), ErrorCode::Service),
    }
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
