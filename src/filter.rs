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

use git2;
use map::OidMap;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path};

#[derive(Debug, Hash)]
pub struct Filter {
    filter: BTreeMap<String, Filter>,
}

impl Filter {
    pub fn new() -> Filter {
        Filter {
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
        let mut filter = Self::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            if line.is_empty() || line.starts_with("#") {
                // Ignore blank lines and comments
                continue;
            }

            filter.insert(Path::new(line));
        }

        Ok(filter)
    }

    /// Inserts a path into the filter. The path is split up and inserted into
    /// the tree.
    pub fn insert(&mut self, path: &Path) {
        let mut components = path.components();

        match components.next() {
            Some(Component::Normal(c)) => {
                let mut filter = self
                    .filter
                    .entry(String::from(c.to_str().unwrap()))
                    .or_insert_with(|| Filter::new());

                // Insert the rest of the components recursively.
                filter.insert(components.as_path());
            }
            _ => {}
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
    pub fn match_entry(&self, entry: &git2::TreeEntry) -> Option<&Filter> {
        for (pattern, filter) in &self.filter {
            if Self::match_name(pattern.as_str(), entry.name().unwrap()) {
                return Some(filter);
            }
        }

        None
    }
}

/// Rewrites a tree such that it only contains the entries specified by the tree
/// filter. This function calls itself recursively to rewrite a tree.
pub fn filter_tree(
    repo: &git2::Repository,
    map: &mut OidMap,
    filter: &Filter,
    tree: &git2::Tree,
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

fn filter_tree_impl(
    repo: &git2::Repository,
    map: &mut OidMap,
    filter: &Filter,
    tree: &git2::Tree,
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
                builder.insert(
                    entry.name_bytes(),
                    entry.id(),
                    entry.filemode(),
                )?;
            } else if entry.kind() == Some(git2::ObjectType::Tree) {
                // There are sub-filters and this is a tree object. Recurse into
                // the tree with the sub-filter for further matching.
                let obj = entry.to_object(repo)?;
                let tree = obj.as_tree().unwrap();

                if let Some(newtree) =
                    filter_tree_impl(repo, map, filter, &tree)?
                {
                    builder.insert(
                        entry.name_bytes(),
                        newtree,
                        entry.filemode(),
                    )?;
                }
            }
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
