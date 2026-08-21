use std::process;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--help"] => print!(
            "Demo CLI\n\nCommands:\n  inspect  inspect input\n  process  process input\n  cwd      print workspace\n\nOptions:\n  --json   JSON output\n  --legacy legacy mode\n"
        ),
        ["inspect", "--json"] => {
            print!(r#"{{"count":3,"mode":"fast","legacy":true}}"#)
        }
        ["process"] => {
            let input = std::fs::read_to_string("input/data.txt").unwrap();
            std::fs::create_dir_all("out").unwrap();
            std::fs::write("out/result.txt", format!("v1:{}", input.trim())).unwrap();
            println!("processed");
        }
        ["cwd"] => print!(
            "\u{1b}[32m{}\u{1b}[0m\r\n",
            std::env::current_dir().unwrap().display()
        ),
        _ => {
            eprintln!("unknown baseline command");
            process::exit(64);
        }
    }
}
