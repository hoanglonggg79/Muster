use std::process::ExitCode;

fn main() -> ExitCode {
    let path = muster_app_lib::bindings_path();

    match muster_app_lib::export_bindings() {
        Ok(()) => {
            println!("đã ghi bindings: {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("lỗi: {error}");
            ExitCode::FAILURE
        }
    }
}
