
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub (in crate::net) struct AddressPair<T> {
    pub source: T,
    pub target: T,
}

impl<T> AddressPair<T> {
    pub fn new(source: T, target: T) -> Self {
        Self { source, target }
    }

    pub fn source(&self) -> &T {
        &self.source
    }

    pub fn target(&self) -> &T {
        &self.target
    }
}

impl<T: Clone> AddressPair<T> {
    pub fn reverse(&self) -> Self {
        Self { source: self.target.clone(), target: self.source.clone() }
    }
}
