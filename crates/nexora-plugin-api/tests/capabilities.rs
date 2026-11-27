//! The manifest's capability set decides what a module may reach.
//!
//! A manifest states what a plugin asks for; the host decides what it gets.
//! The runtime is where those two meet, so the runtime must apply the
//! manifest's capabilities rather than run on the host's policy alone.

use nexora_error::ErrorCode;
use nexora_plugin_api::{PluginCapability, PluginManifest, PluginRuntime, SandboxPolicy};

fn manifest_with(capabilities: Vec<PluginCapability>) -> PluginManifest {
    let mut manifest = PluginManifest::new("demo", "0.1.0", "plugin.wasm");
    for capability in capabilities {
        manifest = manifest.with_capability(capability);
    }
    manifest
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

/// A host policy that allows one host and one directory.
fn host_policy() -> SandboxPolicy {
    let mut policy = SandboxPolicy::deny_all();
    policy.network = true;
    policy.allowed_hosts = vec!["api.example.com".to_owned()];
    policy.filesystem = true;
    policy.allowed_paths = vec!["data".to_owned()];
    policy.time_limit_ms = 1_000;
    policy.memory_limit_mb = 16;
    policy
}

#[test]
fn a_manifest_that_asks_for_a_host_the_host_allows_may_reach_it() {
    let manifest = manifest_with(vec![PluginCapability::Network(
        "api.example.com".to_owned(),
    )]);
    let runtime = PluginRuntime::with_policy(host_policy()).expect("runtime should start");
    runtime
        .call(
            &manifest,
            FETCH_MODULE,
            "fetch",
            &["https://api.example.com/track"],
        )
        .expect("a host both the manifest and the host allow must be reachable");
}

#[test]
fn a_manifest_that_does_not_ask_for_the_network_may_not_reach_it() {
    // The host policy allows the host, but the manifest does not ask for it.
    // The manifest states what the plugin wants, so a plugin that did not ask
    // for the network does not get it.
    let manifest = manifest_with(vec![]);
    let runtime = PluginRuntime::with_policy(host_policy()).expect("runtime should start");
    let error = runtime
        .call(
            &manifest,
            FETCH_MODULE,
            "fetch",
            &["https://api.example.com/track"],
        )
        .expect_err("a host the manifest did not ask for must be refused");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_manifest_that_asks_for_a_host_the_host_does_not_allow_may_not_reach_it() {
    let manifest = manifest_with(vec![PluginCapability::Network(
        "evil.example.org".to_owned(),
    )]);
    let runtime = PluginRuntime::with_policy(host_policy()).expect("runtime should start");
    let error = runtime
        .call(
            &manifest,
            FETCH_MODULE,
            "fetch",
            &["https://evil.example.org/track"],
        )
        .expect_err("a host the host does not allow must be refused");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_manifest_that_asks_for_a_path_the_host_allows_may_reach_it() {
    let manifest = manifest_with(vec![PluginCapability::FileSystem("data".to_owned())]);
    let runtime = PluginRuntime::with_policy(host_policy()).expect("runtime should start");
    let call = runtime
        .call(&manifest, READ_MODULE, "read", &["data/track.bin"])
        .expect("a path both the manifest and the host allow must be reachable");
    assert_eq!(call.results, vec![0]);
}

#[test]
fn a_manifest_that_does_not_ask_for_the_filesystem_may_not_reach_it() {
    let manifest = manifest_with(vec![]);
    let runtime = PluginRuntime::with_policy(host_policy()).expect("runtime should start");
    let error = runtime
        .call(&manifest, READ_MODULE, "read", &["data/track.bin"])
        .expect_err("a path the manifest did not ask for must be refused");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_manifest_that_asks_for_a_path_the_host_does_not_allow_may_not_reach_it() {
    let manifest = manifest_with(vec![PluginCapability::FileSystem("secrets".to_owned())]);
    let runtime = PluginRuntime::with_policy(host_policy()).expect("runtime should start");
    let error = runtime
        .call(&manifest, READ_MODULE, "read", &["secrets/key.bin"])
        .expect_err("a path the host does not allow must be refused");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_path_outside_the_allowed_root_may_not_be_reached() {
    let manifest = manifest_with(vec![PluginCapability::FileSystem("data".to_owned())]);
    let runtime = PluginRuntime::with_policy(host_policy()).expect("runtime should start");
    let error = runtime
        .call(&manifest, READ_MODULE, "read", &["data/../../etc/passwd"])
        .expect_err("a traversing path must be refused");
    assert_eq!(error.code(), ErrorCode::Service);
}

/// A module that calls the host's `read` function with the path it is given.
const READ_MODULE: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x06, 0x01, 0x60, 0x01, 0x7f, 0x01, 0x7f, // type: (i32) -> i32
    0x02, 0x0d, 0x01, // import: host.read
    0x04, b'h', b'o', b's', b't', 0x04, b'r', b'e', b'a', b'd', 0x00, 0x00, 0x03, 0x02, 0x01,
    0x00, // function: one function of type 0
    0x07, 0x08, 0x01, 0x04, b'r', b'e', b'a', b'd', 0x00, 0x00, // export
    0x0a, 0x08, 0x01, 0x06, 0x00, 0x20, 0x00, 0x10, 0x00, 0x0b, // code
];
