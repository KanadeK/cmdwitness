fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cmdwitness::cli::run(&args) {
        Ok(result) => {
            print!("{}", result.stdout);
            std::process::exit(result.exit_code);
        }
        Err(error) => {
            eprintln!("error: {}", error.message);
            std::process::exit(2);
        }
    }
}
