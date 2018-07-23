fn main() {
    println!("Hello world!");
}
// competo start
// competo install bit read_line read_words
pub trait Monoid {
    fn empty() -> Self;
    fn append(&self, right: &Self) -> Self;
}
impl Monoid for i32 {
    fn empty() -> Self {
        0
    }
    fn append(&self, right: &Self) -> Self {
        *self + *right
    }
}
pub struct BIT<T> {
    buf: Vec<T>,
}
impl<T: Monoid + Clone + Sized> BIT<T> {
    pub fn new(len: usize) -> Self {
        BIT {
            buf: vec![T::empty(); len + 1],
        }
    }
    pub fn len(&self) -> usize {
        self.buf.len() - 1
    }
    pub fn add(&mut self, index: usize, value: &T) {
        let mut j = index + 1;
        while j < self.len() {
            self.buf[j] = self.buf[j].append(&value);
            j += rightmost_bit(j);
        }
    }
    pub fn acc(&self, right: usize) -> T {
        let mut acc = T::empty();
        let mut j = right;
        while 0 < j && j < self.len() {
            acc = acc.append(&self.buf[j]);
            j -= rightmost_bit(j);
        }
        acc
    }
}
fn rightmost_bit(n: usize) -> usize {
    let s = n as isize;
    (s & -s) as usize
}
use std::io::{stdin, Read};
pub fn read_line() -> String {
    let mut line = String::new();
    stdin().read_to_string(&mut line).unwrap();
    line.trim_right().to_owned()
}
pub fn read_words() -> Vec<String> {
    let line = read_line();
    line.split_whitespace().map(|s| s.to_owned()).collect()
}
// competo end
