use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let shaders_dir = Path::new("shaders");
    let shared_path = shaders_dir.join("shared.wgsl");
    let shared_src = if shared_path.exists() {
        std::fs::read_to_string(&shared_path).unwrap_or_default()
    } else {
        String::new()
    };

    let passes = [
        "passthrough", "p2m", "m2m", "m2l", "l2l", "l2p", "p2p", "n2_debug",
    ];

    for pass in &passes {
        let pass_path = shaders_dir.join(format!("{}.wgsl", pass));
        if pass_path.exists() {
            let pass_src = std::fs::read_to_string(&pass_path).unwrap();
            let combined = if shared_src.is_empty() {
                pass_src
            } else {
                format!("{}\n{}", shared_src, pass_src)
            };
            let out_path = Path::new(&out_dir).join(format!("{}.wgsl", pass));
            std::fs::write(&out_path, &combined).unwrap();
            println!("cargo::rerun-if-changed={}", pass_path.display());
        }
    }

    println!("cargo::rerun-if-changed={}", shared_path.display());
}
