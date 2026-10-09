fn main() {
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
