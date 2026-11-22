use nexora_plugin_api::{PluginCapability, PluginManifest, SandboxPolicy, validate_manifest};

#[test]
fn valid_manifest_produces_restrictive_sandbox() {
    let manifest = PluginManifest::new("local-adapter", "1.0.0", "plugin.wasm")
        .with_capability(PluginCapability::AudioDecode);
    validate_manifest(&manifest).unwrap();
    let policy = SandboxPolicy::from_manifest(&manifest).unwrap();
    assert!(!policy.network);
    assert!(!policy.filesystem);
    assert_eq!(policy.time_limit_ms, 5_000);
}

#[test]
fn network_capability_must_be_explicit() {
    let manifest = PluginManifest::new("http-adapter", "1.0.0", "plugin.wasm")
        .with_capability(PluginCapability::Network("example.com".into()));
    validate_manifest(&manifest).unwrap();
    let policy = SandboxPolicy::from_manifest(&manifest).unwrap();
    assert!(policy.network);
}

#[test]
fn empty_manifest_is_rejected() {
    let manifest = PluginManifest::new("", "", "");
    assert!(validate_manifest(&manifest).is_err());
}
