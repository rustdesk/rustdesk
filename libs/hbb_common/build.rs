fn main() {
    std::fs::create_dir_all("src/protos").unwrap();
    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir("src/protos")
        .inputs(&["protos/rendezvous.proto", "protos/message.proto"])
        .include("protos")
        .customize(
            protobuf_codegen::Customize::default()
            .tokio_bytes(true)
        )
        .run()
        .expect("Codegen failed.");
}
