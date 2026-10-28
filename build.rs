fn main() {
    println!("cargo:rerun-if-changed=assets/packaging/zeph.rc");
    println!("cargo:rerun-if-changed=assets/app-icon/zeph.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/app-icon/zeph.ico");
        let definition = format!(
            "ZEPH_ICON_PATH=\"{}\"",
            icon.to_string_lossy().replace('\\', "/")
        );
        embed_resource::compile_for("assets/packaging/zeph.rc", ["zeph"], [definition])
            .manifest_required()
            .expect("embed Windows application icon");
    }
    println!("cargo:rerun-if-changed=fixtures/order.proto");
    let mut compiler = protox::Compiler::new(["fixtures"]).expect("include directory");
    compiler
        .include_imports(true)
        .open_file("fixtures/order.proto")
        .expect("example proto");
    tonic_prost_build::configure()
        .compile_fds(compiler.file_descriptor_set())
        .expect("example service");
    std::fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("order.bin"),
        compiler.encode_file_descriptor_set(),
    )
    .expect("write descriptor");
}
