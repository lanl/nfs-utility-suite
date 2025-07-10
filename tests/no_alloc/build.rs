fn main() {
    xdr_codegen::Compiler::new()
        .file("input/structs.x")
        .enable_no_alloc()
        .run()
        .expect("That should have worked. :(");
}
