use std::ffi::CString;
use std::iter::Range;
use std::marker;
use libc::size_t;

use {raw, Error, Tree, PathspecFlags, Index, Repository, DiffDelta, IntoCString};
use util::Binding;

/// Structure representing a compiled pathspec used for matching against various
/// structures.
pub struct Pathspec {
    raw: *mut raw::git_pathspec,
}

/// List of filenames matching a pathspec.
pub struct PathspecMatchList<'ps> {
    raw: *mut raw::git_pathspec_match_list,
    _marker: marker::PhantomData<&'ps Pathspec>,
}

/// Iterator over the matched paths in a pathspec.
pub struct PathspecEntries<'list> {
    range: Range<usize>,
    list: &'list PathspecMatchList<'list>,
}

/// Iterator over the matching diff deltas.
pub struct PathspecDiffEntries<'list> {
    range: Range<usize>,
    list: &'list PathspecMatchList<'list>,
}

/// Iterator over the failed list of pathspec items that did not match.
pub struct PathspecFailedEntries<'list> {
    range: Range<usize>,
    list: &'list PathspecMatchList<'list>,
}

impl Pathspec {
    /// Creates a new pathspec from a list of specs to match against.
    pub fn new<I, T>(specs: I) -> Result<Pathspec, Error>
                     where T: IntoCString, I: Iterator<Item=T> {
        let (_a, _b, arr) = ::util::iter2cstrs(specs);
        unsafe {
            let mut ret = 0 as *mut raw::git_pathspec;
            try_call!(raw::git_pathspec_new(&mut ret, &arr));
            Ok(Binding::from_raw(ret))
        }
    }

    /// Match a pathspec against files in a tree.
    ///
    /// The list returned contains the list of all matched filenames (unless you
    /// pass `PATHSPEC_FAILURES_ONLY` in the flags) and may also contain the
    /// list of pathspecs with no match if the `PATHSPEC_FIND_FAILURES` flag is
    /// specified.
    pub fn match_tree(&self, tree: &Tree, flags: PathspecFlags)
                      -> Result<PathspecMatchList, Error> {
        let mut ret = 0 as *mut raw::git_pathspec_match_list;
        unsafe {
            try_call!(raw::git_pathspec_match_tree(&mut ret, tree.raw(),
                                                   flags.bits(), self.raw));
            Ok(Binding::from_raw(ret))
        }
    }

    /// This matches the pathspec against the files in the repository index.
    ///
    /// The list returned contains the list of all matched filenames (unless you
    /// pass `PATHSPEC_FAILURES_ONLY` in the flags) and may also contain the
    /// list of pathspecs with no match if the `PATHSPEC_FIND_FAILURES` flag is
    /// specified.
    pub fn match_index(&self, index: &Index, flags: PathspecFlags)
                       -> Result<PathspecMatchList, Error> {
        let mut ret = 0 as *mut raw::git_pathspec_match_list;
        unsafe {
            try_call!(raw::git_pathspec_match_index(&mut ret, index.raw(),
                                                    flags.bits(), self.raw));
            Ok(Binding::from_raw(ret))
        }
    }

    /// Match a pathspec against the working directory of a repository.
    ///
    /// This matches the pathspec against the current files in the working
    /// directory of the repository. It is an error to invoke this on a bare
    /// repo. This handles git ignores (i.e. ignored files will not be
    /// considered to match the pathspec unless the file is tracked in the
    /// index).
    ///
    /// The list returned contains the list of all matched filenames (unless you
    /// pass `PATHSPEC_FAILURES_ONLY` in the flags) and may also contain the
    /// list of pathspecs with no match if the `PATHSPEC_FIND_FAILURES` flag is
    /// specified.
    pub fn match_workdir(&self, repo: &Repository, flags: PathspecFlags)
                         -> Result<PathspecMatchList, Error> {
        let mut ret = 0 as *mut raw::git_pathspec_match_list;
        unsafe {
            try_call!(raw::git_pathspec_match_workdir(&mut ret, repo.raw(),
                                                      flags.bits(), self.raw));
            Ok(Binding::from_raw(ret))
        }
    }

    /// Try to match a path against a pathspec
    ///
    /// Unlike most of the other pathspec matching functions, this will not fall
    /// back on the native case-sensitivity for your platform. You must
    /// explicitly pass flags to control case sensitivity or else this will fall
    /// back on being case sensitive.
    pub fn matches_path(&self, path: &Path, flags: PathspecFlags) -> bool {
        let path = CString::from_slice(path.as_vec());
        unsafe {
            raw::git_pathspec_matches_path(&*self.raw, flags.bits(),
                                           path.as_ptr()) == 1
        }
    }
}

impl Binding for Pathspec {
    type Raw = *mut raw::git_pathspec;

    unsafe fn from_raw(raw: *mut raw::git_pathspec) -> Pathspec {
        Pathspec { raw: raw }
    }
    fn raw(&self) -> *mut raw::git_pathspec { self.raw }
}

#[unsafe_destructor]
impl Drop for Pathspec {
    fn drop(&mut self) {
        unsafe { raw::git_pathspec_free(self.raw) }
    }
}

impl<'ps> PathspecMatchList<'ps> {
    fn entrycount(&self) -> usize {
        unsafe { raw::git_pathspec_match_list_entrycount(&*self.raw) as usize }
    }

    fn failed_entrycount(&self) -> usize {
        unsafe { raw::git_pathspec_match_list_failed_entrycount(&*self.raw) as usize }
    }

    /// Returns an iterator over the matching filenames in this list.
    pub fn entries(&self) -> PathspecEntries {
        let n = self.entrycount();
        let n = if n > 0 && self.entry(0).is_none() {0} else {n};
        PathspecEntries { range: range(0, n), list: self }
    }

    /// Get a matching filename by position.
    ///
    /// If this list was generated from a diff, then the return value will
    /// always be `None.
    pub fn entry(&self, i: usize) -> Option<&[u8]> {
        unsafe {
            let ptr = raw::git_pathspec_match_list_entry(&*self.raw, i as size_t);
            ::opt_bytes(self, ptr)
        }
    }

    /// Returns an iterator over the matching diff entries in this list.
    pub fn diff_entries(&self) -> PathspecDiffEntries {
        let n = self.entrycount();
        let n = if n > 0 && self.diff_entry(0).is_none() {0} else {n};
        PathspecDiffEntries { range: range(0, n), list: self }
    }

    /// Get a matching diff delta by position.
    ///
    /// If the list was not generated from a diff, then the return value will
    /// always be `None`.
    pub fn diff_entry(&self, i: usize) -> Option<DiffDelta> {
        unsafe {
            let ptr = raw::git_pathspec_match_list_diff_entry(&*self.raw,
                                                              i as size_t);
            Binding::from_raw_opt(ptr as *mut _)
        }
    }

    /// Returns an iterator over the non-matching entries in this list.
    pub fn failed_entries(&self) -> PathspecFailedEntries {
        let n = self.failed_entrycount();
        let n = if n > 0 && self.failed_entry(0).is_none() {0} else {n};
        PathspecFailedEntries { range: range(0, n), list: self }
    }

    /// Get an original pathspec string that had no matches.
    pub fn failed_entry(&self, i: usize) -> Option<&[u8]> {
        unsafe {
            let ptr = raw::git_pathspec_match_list_failed_entry(&*self.raw,
                                                                i as size_t);
            ::opt_bytes(self, ptr)
        }
    }
}

impl<'ps> Binding for PathspecMatchList<'ps> {
    type Raw = *mut raw::git_pathspec_match_list;

    unsafe fn from_raw(raw: *mut raw::git_pathspec_match_list)
                       -> PathspecMatchList<'ps> {
        PathspecMatchList { raw: raw, _marker: marker::PhantomData }
    }
    fn raw(&self) -> *mut raw::git_pathspec_match_list { self.raw }
}

#[unsafe_destructor]
impl<'ps> Drop for PathspecMatchList<'ps> {
    fn drop(&mut self) {
        unsafe { raw::git_pathspec_match_list_free(self.raw) }
    }
}

impl<'list> Iterator for PathspecEntries<'list> {
    type Item = &'list [u8];
    fn next(&mut self) -> Option<&'list [u8]> {
        self.range.next().and_then(|i| self.list.entry(i))
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.range.size_hint() }
}
impl<'list> DoubleEndedIterator for PathspecEntries<'list> {
    fn next_back(&mut self) -> Option<&'list [u8]> {
        self.range.next_back().and_then(|i| self.list.entry(i))
    }
}
impl<'list> ExactSizeIterator for PathspecEntries<'list> {}

impl<'list> Iterator for PathspecDiffEntries<'list> {
    type Item = DiffDelta<'list>;
    fn next(&mut self) -> Option<DiffDelta<'list>> {
        self.range.next().and_then(|i| self.list.diff_entry(i))
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.range.size_hint() }
}
impl<'list> DoubleEndedIterator for PathspecDiffEntries<'list> {
    fn next_back(&mut self) -> Option<DiffDelta<'list>> {
        self.range.next_back().and_then(|i| self.list.diff_entry(i))
    }
}
impl<'list> ExactSizeIterator for PathspecDiffEntries<'list> {}

impl<'list> Iterator for PathspecFailedEntries<'list> {
    type Item = &'list [u8];
    fn next(&mut self) -> Option<&'list [u8]> {
        self.range.next().and_then(|i| self.list.failed_entry(i))
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.range.size_hint() }
}
impl<'list> DoubleEndedIterator for PathspecFailedEntries<'list> {
    fn next_back(&mut self) -> Option<&'list [u8]> {
        self.range.next_back().and_then(|i| self.list.failed_entry(i))
    }
}
impl<'list> ExactSizeIterator for PathspecFailedEntries<'list> {}

#[cfg(test)]
mod tests {
    use PATHSPEC_DEFAULT;
    use super::Pathspec;
    use std::old_io::File;

    #[test]
    fn smoke() {
        let ps = Pathspec::new(["a"].iter()).unwrap();
        assert!(ps.matches_path(&Path::new("a"), PATHSPEC_DEFAULT));
        assert!(ps.matches_path(&Path::new("a/b"), PATHSPEC_DEFAULT));
        assert!(!ps.matches_path(&Path::new("b"), PATHSPEC_DEFAULT));
        assert!(!ps.matches_path(&Path::new("ab/c"), PATHSPEC_DEFAULT));

        let (td, repo) = ::test::repo_init();
        let list = ps.match_workdir(&repo, PATHSPEC_DEFAULT).unwrap();
        assert_eq!(list.entries().len(), 0);
        assert_eq!(list.diff_entries().len(), 0);
        assert_eq!(list.failed_entries().len(), 0);

        File::create(&td.path().join("a")).unwrap();

        let list = ps.match_workdir(&repo, ::PATHSPEC_FIND_FAILURES).unwrap();
        assert_eq!(list.entries().len(), 1);
        assert_eq!(list.entries().next(), Some(b"a"));
    }
}
