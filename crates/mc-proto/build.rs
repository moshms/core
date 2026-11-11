use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("PROTOC").is_none()
        && Command::new("protoc").arg("--version").output().is_err()
    {
        std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    }

    let proto_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let proto = proto_dir.join("transcoder.proto");

    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&[&proto], &[&proto_dir])?;

    println!("cargo:rerun-if-changed={}", proto.display());
    Ok(())
}
