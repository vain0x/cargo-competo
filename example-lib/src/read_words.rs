use read_line::*;

pub fn read_words() -> Vec<String> {
    let line = read_line();
    line.split_whitespace().map(|s| s.to_owned()).collect()
}
