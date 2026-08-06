use std::{boxed::Box, error::KernelError, kerror, vec::Vec};

///A wrapper type for path, that have been resolved to a list of path components
///That is, the path starts from root and does not contain any "." or ".." components
#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolvedPath(Box<[Box<str>]>);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPathBorrowed<'a>(&'a [Box<str>]);

impl<'a> core::convert::From<&'a ResolvedPath> for ResolvedPathBorrowed<'a> {
    fn from(value: &'a ResolvedPath) -> Self {
        ResolvedPathBorrowed(&value.0)
    }
}

impl<'a> core::convert::From<&ResolvedPathBorrowed<'a>> for ResolvedPathBorrowed<'a> {
    fn from(value: &ResolvedPathBorrowed<'a>) -> Self {
        ResolvedPathBorrowed(value.0)
    }
}

impl ResolvedPath {
    pub fn new(path: Box<[Box<str>]>) -> Self {
        ResolvedPath(path)
    }

    pub fn root() -> Self {
        ResolvedPath(Box::new([]))
    }

    pub fn index(&self, range: core::ops::Range<usize>) -> ResolvedPathBorrowed<'_> {
        ResolvedPathBorrowed(&self.0[range])
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.0.get(index).map(|s| s.as_ref())
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Box<str>> {
        self.0.iter()
    }

    pub fn take(self) -> Box<[Box<str>]> {
        self.0
    }

    pub fn inner(&self) -> &[Box<str>] {
        &self.0
    }
}

impl ResolvedPathBorrowed<'_> {
    pub fn index(&self, range: core::ops::Range<usize>) -> ResolvedPathBorrowed<'_> {
        ResolvedPathBorrowed(&self.0[range])
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.0.get(index).map(|s| s.as_ref())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Box<str>> {
        self.0.iter()
    }

    pub fn inner(&self) -> &[Box<str>] {
        self.0
    }
}

impl core::fmt::Display for ResolvedPathBorrowed<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for component in self.0.iter() {
            write!(f, "/{}", component)?;
        }
        if self.0.is_empty() {
            write!(f, "/")?;
        }
        Ok(())
    }
}

pub fn resolve_path(path: &str) -> ResolvedPath {
    let chunks = path.split('/');
    let mut path = Vec::new();
    for chunk in chunks {
        //according to posix multiple slashes are the same as a single slash. Ignore
        if chunk.is_empty() {
            continue;
        }
        if chunk == "." {
            continue;
        }
        path.push(chunk.into());
    }

    ResolvedPath::new(path.into())
}

pub fn validate_path<'a>(path: &'a [Box<str>]) -> Result<ResolvedPathBorrowed<'a>, KernelError> {
    for component in path.iter() {
        if component.is_empty() {
            return kerror!(InvalidArgument);
        }
        if component.contains('/') {
            return kerror!(InvalidArgument);
        }
        if **component == *"." || **component == *".." {
            panic!("idk");
            // return kerror!(InvalidArgument);
        }
    }

    Ok(ResolvedPathBorrowed(path))
}
