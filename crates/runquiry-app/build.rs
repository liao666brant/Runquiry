//! 将应用图标嵌入 Windows 产品二进制。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../assets/icons/runquiry.ico");
    println!("cargo:rerun-if-changed=resources/runquiry.rc");
    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows" {
        embed_resource::compile_for("resources/runquiry.rc", ["runquiry"], embed_resource::NONE)
            .manifest_required()?;
    }
    Ok(())
}
