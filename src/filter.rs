// Copyright (c) 2017 Jason White
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::map::OidMap;
use git2::{self, Oid, TreeBuilder, TreeEntry};

use std::collections::{btree_map, BTreeMap};
use std::fs;
use std::io;
use std::path::{Component, Components, Path};

struct PathIterator<'a> {
    components: Components<'a>,
}

impl<'a> PathIterator<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            components: path.components(),
        }
    }
}

impl<'a> Iterator for PathIterator<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.components.next().and_then(|c| match c {
            Component::Normal(c) => Some(String::from(c.to_str().unwrap())),
            _ => None,
        })
    }
}

#[derive(Debug, Hash)]
pub struct Filter {
    exclude: bool,
    filter: BTreeMap<String, Filter>,
}

impl Filter {
    pub fn new(exclude: bool) -> Filter {
        Filter {
            exclude,
            filter: BTreeMap::new(),
        }
    }

    /// Load from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Filter> {
        Self::from_reader(io::BufReader::new(fs::File::open(path)?))
    }

    /// Load from a reader. The file shall consist of lines containing paths.
    /// Blank lines and lines starting with a "#" are ignored.
    pub fn from_reader<R: io::BufRead>(reader: R) -> io::Result<Filter> {
        let mut filter: Filter = Default::default();
        let mut exclude = false;

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            if line == "# !EXCLUDES!" {
                exclude = true;
            } else if line.is_empty() || line.starts_with("#") {
                // Ignore blank lines and comments
                continue;
            }

            let path = Path::new(line);
            if exclude {
                filter.insert_exclude(path);
            } else {
                filter.insert_include(path);
            }
        }

        Ok(filter)
    }

    /// Inserts a path into the filter. The path is split up and inserted into
    /// the tree.
    pub fn insert_include(&mut self, path: &Path) {
        let mut components = PathIterator::new(path);

        let mut filter = self;
        while let Some(component) = components.next() {
            if filter.exclude {
                // Component cannot be included when it has been previously excluded
                break;
            }
            filter = filter
                .filter
                .entry(component)
                .or_insert_with(|| Filter::new(false));
        }
        filter.filter.clear();
    }

    fn insert_excluded_components(
        &mut self,
        mut component: String,
        mut components: PathIterator<'_>,
    ) {
        let mut filter = self;
        loop {
            match filter.filter.entry(component) {
                btree_map::Entry::Occupied(entry) => {
                    filter = entry.into_mut();
                    if filter.is_empty() {
                        break;
                    }
                }
                btree_map::Entry::Vacant(entry) => {
                    filter = entry.insert(Filter::new(true))
                }
            }

            if let Some(component_next) = components.next() {
                component = component_next;
            } else {
                break;
            }
        }
        filter.filter.clear();
    }

    pub fn insert_exclude(&mut self, path: &Path) {
        let mut components = PathIterator::new(path);

        let mut filter = self;
        while let Some(component) = components.next() {
            if filter.is_empty() {
                filter.exclude = true;
                filter.insert_excluded_components(component, components);
                break;
            } else if filter.exclude {
                filter.insert_excluded_components(component, components);
                break;
            }
            match filter.filter.entry(component) {
                btree_map::Entry::Occupied(entry) => {
                    filter = entry.into_mut();
                }
                btree_map::Entry::Vacant(_) => break,
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.filter.is_empty()
    }

    pub fn match_name(pattern: &str, name: &str) -> bool {
        // TODO: Do proper pattern matching. This will complicate the
        // implementation a bit.
        pattern == "" || pattern == "**" || pattern == name
    }

    /// Attempts to match a `TreeEntry` for each of the filters. If one matches,
    /// returns a reference to that filter.
    ///
    /// FIXME: When glob pattern matching is implemented, there may be multiple
    /// filters that can match. It would be better to return an iterator of the
    /// matching filters.
    pub fn match_entry(&self, entry: &git2::TreeEntry<'_>) -> Option<&Filter> {
        for (pattern, filter) in &self.filter {
            if Self::match_name(pattern.as_str(), entry.name().unwrap()) {
                return Some(filter);
            }
        }

        None
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter::new(false)
    }
}

/// Rewrites a tree such that it only contains the entries specified by the tree
/// filter. This function calls itself recursively to rewrite a tree.
pub fn filter_tree(
    repo: &git2::Repository,
    map: &mut OidMap,
    filter: &Filter,
    tree: &git2::Tree<'_>,
) -> Result<git2::Oid, git2::Error> {
    match filter_tree_impl(repo, map, filter, tree)? {
        Some(oid) => Ok(oid),

        // The tree is entirely empty. Building this tree will always yield the
        // empty tree hash "4b825dc642cb6eb9a060e54bf8d69288fbee4904". Since we
        // should only create an empty tree for the root tree (not subtrees), we
        // don't do this in the recursive impl.
        None => repo.treebuilder(None)?.write(),
    }
}

fn insert_entry_to_builder(
    builder: &mut TreeBuilder,
    entry: TreeEntry,
    newtree: Option<Oid>,
) -> Result<(), git2::Error> {
    builder
        .insert(
            entry.name_bytes(),
            newtree.map_or_else(|| entry.id(), |oid| oid),
            entry.filemode(),
        )
        .map(|_| ())
}

fn filter_tree_impl(
    repo: &git2::Repository,
    map: &mut OidMap,
    filter: &Filter,
    tree: &git2::Tree<'_>,
) -> Result<Option<git2::Oid>, git2::Error> {
    if let Some(oid) = map.get(&tree.id()) {
        // The work has already been done. Skip it.
        return Ok(*oid);
    }

    let mut builder = repo.treebuilder(None)?;

    for entry in tree {
        if let Some(filter) = filter.match_entry(&entry) {
            if filter.is_empty() {
                // There are no sub-filters. Match this tree entirely.
                if !filter.exclude {
                    insert_entry_to_builder(&mut builder, entry, None)?;
                }
            } else if entry.kind() == Some(git2::ObjectType::Tree) {
                // There are sub-filters and this is a tree object. Recurse into
                // the tree with the sub-filter for further matching.
                let obj = entry.to_object(repo)?;
                let tree = obj.as_tree().unwrap();

                if let Some(newtree) =
                    filter_tree_impl(repo, map, filter, &tree)?
                {
                    insert_entry_to_builder(
                        &mut builder,
                        entry,
                        Some(newtree),
                    )?;
                }
            }
        } else if filter.exclude {
            // There is no match for exclude. Match this tree entirely.
            insert_entry_to_builder(&mut builder, entry, None)?;
        }
    }

    if builder.len() == 0 {
        // There are no entries in this tree. Don't write it out.
        Ok(None)
    } else {
        let oid = builder.write()?;

        // Cache it.
        map.insert(tree.id(), Some(oid));

        Ok(Some(oid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FilterFlatterer {
        stack: Vec<String>,
        paths: Vec<String>,
    }

    impl FilterFlatterer {
        fn flatten_impl(&mut self, filter: &Filter) {
            for (pattern, filter) in &filter.filter {
                let component = if filter.exclude {
                    format!("{}*", pattern)
                } else {
                    pattern.clone()
                };
                self.stack.push(component);
                if filter.is_empty() {
                    self.paths.push(self.stack.join("/"));
                } else {
                    self.flatten_impl(filter);
                }
                self.stack.pop();
            }
        }

        fn flatten(mut self, filter: &Filter) -> Vec<String> {
            if filter.exclude {
                self.stack.push("*".into());
            }
            self.flatten_impl(filter);
            self.paths
        }
    }

    impl Default for FilterFlatterer {
        fn default() -> Self {
            Self {
                stack: Default::default(),
                paths: Default::default(),
            }
        }
    }

    fn check_filter(filter: &Filter, mut ref_paths: Vec<&str>) {
        let mut filter_paths = FilterFlatterer::default().flatten(&filter);

        filter_paths.sort();
        ref_paths.sort();

        assert_eq!(filter_paths, ref_paths);
    }

    #[test]
    fn insert_includes_empty() {
        let filter: Filter = Default::default();
        check_filter(&filter, vec![]);
    }

    #[test]
    fn insert_includes() {
        let mut filter: Filter = Default::default();

        filter.insert_include(Path::new("a/b/c"));
        filter.insert_include(Path::new("a/b"));
        filter.insert_include(Path::new("b"));
        filter.insert_include(Path::new("a/b/d"));
        filter.insert_include(Path::new("a/b/e"));
        filter.insert_include(Path::new("a/c/d/e"));
        filter.insert_include(Path::new("a/c/d/f"));
        filter.insert_include(Path::new("a/c/d"));

        check_filter(&filter, vec!["a/b/d", "a/b/e", "a/c/d", "b"]);
    }

    #[test]
    fn insert_excludes() {
        let mut filter: Filter = Default::default();

        filter.insert_exclude(Path::new("a/b/c"));
        filter.insert_exclude(Path::new("a/b"));
        filter.insert_exclude(Path::new("b"));
        filter.insert_exclude(Path::new("c/d"));

        check_filter(&filter, vec!["*/a*/b*", "*/b*", "*/c*/d*"]);
    }

    #[test]
    fn insert_mixed() {
        let mut filter: Filter = Default::default();

        filter.insert_include(Path::new("a/b"));
        filter.insert_exclude(Path::new("a/b/c"));
        filter.insert_exclude(Path::new("a/b/d"));
        filter.insert_exclude(Path::new("a/c/d"));
        filter.insert_include(Path::new("b"));
        filter.insert_include(Path::new("c/d/e/f"));
        filter.insert_exclude(Path::new("c/d/e"));
        filter.insert_include(Path::new("d/e/f"));
        filter.insert_exclude(Path::new("d/e/f/g/h"));
        filter.insert_exclude(Path::new("e"));
        filter.insert_include(Path::new("f"));
        filter.insert_exclude(Path::new("f/g/h"));
        filter.insert_exclude(Path::new("f/g"));

        check_filter(
            &filter,
            vec![
                "a/b*/c*",
                "a/b*/d*",
                "b",
                "c/d/e/f",
                "d/e/f*/g*/h*",
                "f*/g*",
            ],
        );
    }
}
