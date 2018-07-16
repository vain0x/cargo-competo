use monoid::Monoid;

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

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut bit = BIT::new(6);
        for (i, x) in [3, 1, 4, 1, 5, 9].into_iter().enumerate() {
            bit.add(i, x);
        }

        assert_eq!(3 + 1 + 4, bit.acc(3));
    }
}
