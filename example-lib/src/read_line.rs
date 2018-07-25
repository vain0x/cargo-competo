use std::io::{stdin, Read};

pub fn read_line() -> String {
    let mut line = String::new();
    stdin().read_to_string(&mut line).unwrap();
    line.trim_right().to_owned()
}
