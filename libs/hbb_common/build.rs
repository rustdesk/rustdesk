fn main() {
    std::fs::create_dir_all("src/protos").unwrap();
    protobuf_codegen_pure::Codegen::new()
        .out_dir("src/protos")
        .inputs(&["protos/rendezvous.proto", "protos/message.proto"])
        .include("protos")
        .run()
        .expect("Codegen failed.");
}
