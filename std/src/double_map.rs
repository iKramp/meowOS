
struct DoubleMap<T, U> {
    map_t_to_u: crate::collections::BTreeMap<T, U>,
    map_u_to_t: crate::collections::BTreeMap<U, T>,
}

impl<T, U> DoubleMap<T, U>
where
    T: Ord + Clone,
    U: Ord + Clone,
{
    pub fn new() -> Self {
        DoubleMap {
            map_t_to_u: crate::collections::BTreeMap::new(),
            map_u_to_t: crate::collections::BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, t: T, u: U) -> (Option<T>, Option<U>) {
        let prev_u = self.map_t_to_u.insert(t.clone(), u.clone());
        let prev_t = self.map_u_to_t.insert(u, t);
        (prev_t, prev_u)
    }

    pub fn get_by_t(&self, t: &T) -> Option<&U> {
        self.map_t_to_u.get(t)
    }

    pub fn get_by_u(&self, u: &U) -> Option<&T> {
        self.map_u_to_t.get(u)
    }

    pub fn clear(&mut self) {
        self.map_t_to_u.clear();
        self.map_u_to_t.clear();
    }
}
