fn main() {
    println!("cargo:rerun-if-changed=../../packaging/windows/rmux.rc");
    println!("cargo:rerun-if-changed=../../docs/brand/rmux.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // GPUI loads the application's icon from resource ID 1.
        embed_resource::compile_for(
            "../../packaging/windows/rmux.rc",
            ["rmux"],
            embed_resource::NONE,
        ).manifest_required().expect("compile Windows application icon");
    }
}
