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
