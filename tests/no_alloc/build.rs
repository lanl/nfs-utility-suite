fn main() {
    xdr_codegen::Compiler::new()
        .file("input/structs.x")
        .file("input/arrays.x")
        .file("input/optional.x")
        .file("input/unions.x")
        .enable_no_alloc()
        .disable_alloc()
        .run()
        .expect("That should have worked. :(");
}
