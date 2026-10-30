fn main() -> std::process::ExitCode {
    match chronoscope::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("chronoscope: {err}");
            std::process::ExitCode::FAILURE
        }
    }
}
