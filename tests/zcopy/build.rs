fn main() {
    xdr_codegen::Compiler::new()
        .file("input/hello.x")
        .file("input/structs.x")
        .enable_zcopy()
        .run()
        .expect("That should have worked. :(");
}
