fn main() {
    xdr_codegen::Compiler::new()
        .file("input/arrays.x")
        .file("input/hello.x")
        .file("input/typedef.x")
        .file("input/unions.x")
        .file("input/structs.x")
        .file("input/optional.x")
        .run()
        .expect("That should have worked. :(");
}
