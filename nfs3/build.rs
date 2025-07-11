fn main() {
    xdr_codegen::Compiler::new()
        .file("mount_proto.x")
        .run()
        .expect("That should have worked. :(");
}
