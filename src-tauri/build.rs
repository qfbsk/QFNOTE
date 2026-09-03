fn main() {
    
    let sdk_path = r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64";
    if let Ok(current_path) = std::env::var("PATH") {
        let new_path = format!("{};{}", sdk_path, current_path);
        std::env::set_var("PATH", new_path);
    }
    
    tauri_build::build();

    
    
    println!("cargo:rerun-if-changed=../dist");
}
