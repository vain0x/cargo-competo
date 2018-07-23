fn read_line() -> String {
    use std::io::stdin;
    let mut line = String::new();
    stdin().read_to_string(&mut line).unwrap();
    line.trim_right().to_owned()
}
