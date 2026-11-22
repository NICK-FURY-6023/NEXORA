fn main() {
    let protoc =
        protoc_bin_vendored::protoc_bin_path().expect("vendored protoc binary must be available");
    let mut config = tonic_prost_build::Config::new();
    config.protoc_executable(protoc);
    tonic_prost_build::configure()
        .compile_with_config(config, &["proto/nexora.proto"], &["proto"])
        .expect("compile nexora.proto");
    println!("cargo:rerun-if-changed=proto/nexora.proto");
}
