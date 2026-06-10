fn main() {
    xdr_codegen::Compiler::new()
        .file("input/hello.x")
        .file("input/structs.x")
        .file("input/arrays.x")
        .file("input/optional.x")
        .enable_zcopy()
        .run()
        .expect("That should have worked. :(");
}
