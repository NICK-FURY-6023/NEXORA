use std::fs;
use std::path::Path;
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

#[test]
fn the_query_cannot_decide_the_host() {
    let error = network_sandbox()
        .check_network("https://evil.example.org/track?next=api.example.com")
        .expect_err("the query must not decide the host");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn the_fragment_cannot_decide_the_host() {
    let error = network_sandbox()
        .check_network("https://evil.example.org/track#api.example.com")
        .expect_err("the fragment must not decide the host");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn userinfo_cannot_decide_the_host() {
    let error = network_sandbox()
        .check_network("https://api.example.com@evil.com/track")
        .expect_err("the userinfo must not decide the host");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn a_backslash_cannot_decide_the_host() {
    // A backslash ends the authority for an http(s) URL, so the host is what
    // precedes it. A guard that reads the whole authority would see the
    // allowed name and let the URL through to a host the fetcher never
    // reaches. The URL is malformed, so it is refused as invalid input.
    for url in [
        r"https://evil.example.org\@api.example.com/track",
        r"https://evil.example.org\api.example.com/track",
    ] {
        let error = network_sandbox()
            .check_network(url)
            .expect_err("the backslash must not decide the host");
        assert_eq!(
            error.code(),
            ErrorCode::InvalidInput,
            "{url} must be denied"
        );
    }
}

fn loopback_sandbox(host: &str) -> PluginSandbox {
    PluginSandbox::from_manifest(&manifest_with(vec![PluginCapability::Network(
        host.to_owned(),
    )]))
    .expect("valid manifest")
}

#[test]
fn a_numeric_loopback_host_is_denied() {
    for host in [
        "2130706433",
        "0x7f000001",
        "127.1",
        "127.0.0.1.",
        "::ffff:127.0.0.1",
    ] {
        let error = loopback_sandbox(host)
            .check_network(&format!("https://{host}/track"))
            .expect_err("a loopback host must be denied");
        assert_eq!(error.code(), ErrorCode::Service, "{host} must be denied");
    }
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
fn grants_refuses_a_traversing_filesystem_path() {
    assert!(!filesystem_sandbox().grants(&PluginCapability::FileSystem(
        "data/../../etc/passwd".to_owned()
    )));
}

#[test]
fn the_exposed_capability_set_follows_the_effective_policy() {
    let mut approved = SandboxPolicy::deny_all();
    approved.network = true;
    approved.allowed_hosts = vec!["api.example.com".to_owned()];
    approved.time_limit_ms = 1_000;
    approved.memory_limit_mb = 16;

    let manifest = manifest_with(vec![
        PluginCapability::AudioDecode,
        PluginCapability::Network("api.example.com".to_owned()),
        PluginCapability::Network("evil.example.org".to_owned()),
    ]);
    let sandbox =
        PluginSandbox::from_manifest_with(&manifest, &approved).expect("manifest should be valid");

    // The host dropped one host, so the plugin must not be shown a capability
    // for it: a consumer that reads the exposed set would otherwise hand out
    // access the sandbox refuses.
    assert!(
        sandbox
            .capabilities()
            .contains(&PluginCapability::Network("api.example.com".to_owned()))
    );
    assert!(
        !sandbox
            .capabilities()
            .contains(&PluginCapability::Network("evil.example.org".to_owned()))
    );
    assert!(
        sandbox
            .capabilities()
            .contains(&PluginCapability::AudioDecode)
    );
}

#[test]
fn a_private_host_is_never_granted_or_exposed() {
    let manifest = manifest_with(vec![PluginCapability::Network("127.0.0.1".to_owned())]);
    let sandbox = PluginSandbox::from_manifest(&manifest).expect("manifest should be valid");

    assert!(!sandbox.grants(&PluginCapability::Network("127.0.0.1".to_owned())));
    assert!(
        !sandbox
            .capabilities()
            .contains(&PluginCapability::Network("127.0.0.1".to_owned()))
    );
}

#[test]
fn a_limit_cannot_be_raised_after_the_sandbox_is_built() {
    let mut policy = SandboxPolicy::deny_all();
    policy.memory_limit_mb = 1;
    let mut sandbox = PluginSandbox::new(policy, Vec::new());
    sandbox.set_memory_limit_mb(u64::MAX);
    let error = sandbox
        .allocate(2 * 1024 * 1024)
        .expect_err("the memory limit must not be raised");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[test]
fn the_time_limit_cannot_be_raised_after_the_sandbox_is_built() {
    let mut policy = SandboxPolicy::deny_all();
    policy.time_limit_ms = 0;
    let mut sandbox = PluginSandbox::new(policy, Vec::new());
    std::thread::sleep(Duration::from_millis(5));
    sandbox.set_time_limit_ms(60_000);
    let error = sandbox
        .check_time()
        .expect_err("the time limit must not be raised");
    assert_eq!(error.code(), ErrorCode::Service);
}

/// The eight-byte header of a WebAssembly module: the magic and version 1.
fn wasm_header() -> &'static [u8] {
    b"\0asm\x01\x00\x00\x00"
}

#[test]
fn loading_a_module_resolves_the_entrypoint_inside_the_root() {
    let root = std::env::temp_dir().join(format!("nexora-plugin-load-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temp root should exist");
    fs::write(root.join("plugin.wasm"), wasm_header()).expect("module should be written");

    let manifest = manifest_with(vec![PluginCapability::FileSystem("data".to_owned())]);
    let module = PluginSandbox::from_manifest(&manifest)
        .expect("manifest should be valid")
        .load(&root, &manifest)
        .expect("module should load");
    let expected = std::fs::canonicalize(root.join("plugin.wasm")).expect("canonical path");
    assert_eq!(module.entrypoint(), expected);
    assert_eq!(module.bytes(), wasm_header());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn loading_a_module_rejects_a_file_that_is_not_webassembly() {
    let root = std::env::temp_dir().join(format!("nexora-plugin-notwasm-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temp root should exist");
    fs::write(root.join("plugin.wasm"), b"not a wasm module at all")
        .expect("module should be written");

    let manifest = manifest_with(vec![]);
    let error = PluginSandbox::from_manifest(&manifest)
        .expect("manifest should be valid")
        .load(&root, &manifest)
        .expect_err("a file that is not webassembly must be rejected");
    assert_eq!(error.code(), ErrorCode::InvalidInput);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn loading_a_module_rejects_an_unsupported_webassembly_version() {
    let root = std::env::temp_dir().join(format!("nexora-plugin-version-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temp root should exist");
    let mut header = wasm_header().to_vec();
    header[4] = 2;
    fs::write(root.join("plugin.wasm"), &header).expect("module should be written");

    let manifest = manifest_with(vec![]);
    let error = PluginSandbox::from_manifest(&manifest)
        .expect("manifest should be valid")
        .load(&root, &manifest)
        .expect_err("an unsupported wasm version must be rejected");
    assert_eq!(error.code(), ErrorCode::InvalidInput);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn loading_a_module_rejects_a_symlink_out_of_the_root() {
    let root = std::env::temp_dir().join(format!("nexora-plugin-symlink-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temp root should exist");
    let outside =
        std::env::temp_dir().join(format!("nexora-plugin-outside-{}.wasm", std::process::id()));
    fs::write(&outside, wasm_header()).expect("module should be written");

    if symlink_file(&outside, &root.join("plugin.wasm")).is_err() {
        // The platform does not let this user create symlinks.
        fs::remove_dir_all(&root).ok();
        fs::remove_file(&outside).ok();
        return;
    }

    let manifest = manifest_with(vec![]);
    let error = PluginSandbox::from_manifest(&manifest)
        .expect("manifest should be valid")
        .load(&root, &manifest)
        .expect_err("a symlink out of the root must be rejected");
    assert_eq!(error.code(), ErrorCode::InvalidInput);

    fs::remove_dir_all(&root).ok();
    fs::remove_file(&outside).ok();
}

#[test]
fn the_host_policy_narrows_what_the_plugin_may_reach() {
    let mut approved = SandboxPolicy::deny_all();
    approved.network = true;
    approved.allowed_hosts = vec!["api.example.com".to_owned()];
    approved.time_limit_ms = 1_000;
    approved.memory_limit_mb = 16;

    let manifest = manifest_with(vec![
        PluginCapability::Network("api.example.com".to_owned()),
        PluginCapability::Network("evil.example.org".to_owned()),
    ]);
    let sandbox =
        PluginSandbox::from_manifest_with(&manifest, &approved).expect("manifest should be valid");

    sandbox
        .check_network("https://api.example.com/track")
        .expect("the approved host stays allowed");
    let error = sandbox
        .check_network("https://evil.example.org/track")
        .expect_err("an unapproved host must be denied");
    assert_eq!(error.code(), ErrorCode::Service);
}

#[cfg(windows)]
fn symlink_file(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(source, target)
}

#[cfg(unix)]
fn symlink_file(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
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
