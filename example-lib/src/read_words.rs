use read_line::read_line;

fn read_words() -> Vec<String> {
    let line = read_line();
    line.split_whitespace().collect()
}
