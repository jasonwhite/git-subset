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

use std::collections::HashMap;
use std::fs;
use std::io;

use git2::{Oid, Repository};

/// An OID mapping. This is simply a mapping between original commit hashes and
/// rewritten commit hashes.
///
/// If an OID maps to `None`, it means that the object was discarded.
///
/// *Note*: The sets of keys and values are not guaranteed to be disjoint. That
/// is, a mapping like "A -> B -> C" may be possible. In such a case, looking up
/// a key will resolve to the deepest value. This is so that pruning empty
/// commits works correctly when remapping parents.
#[derive(Debug)]
pub struct OidMap {
    map: HashMap<Oid, Option<Oid>>,
}

impl OidMap {
    pub fn new() -> OidMap {
        OidMap {
            map: HashMap::new(),
        }
    }

    /// Reads the map from a file inside the given repository. The name of the
    /// file is derived from the hash of the file filter. Thus, when the file
    /// filter changes, we also get a different map.
    pub fn from_repo(repo: &Repository, name: &str) -> io::Result<OidMap> {
        let mut path = repo.path().join("subset");
        path.push(name);

        if let Ok(f) = fs::File::open(&path) {
            Self::from_reader(io::BufReader::new(f))
        } else {
            Ok(Self::new())
        }
    }

    /// Writes the map to a file inside the given repository.
    pub fn write_repo(&self, repo: &Repository, name: &str) -> io::Result<()> {
        let mut path = repo.path().join("subset");

        fs::create_dir_all(&path)?;

        path.push(name);

        let mut f = io::BufWriter::new(fs::File::create(&path)?);
        self.write(&mut f)
    }

    /// Reads the mapping from a file.
    pub fn from_reader<R: io::BufRead>(reader: R) -> io::Result<OidMap> {
        let mut map = HashMap::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            if line.is_empty() || line.starts_with("#") {
                // Ignore blank lines and comments.
                continue;
            }

            let mut s = line.split(' ');

            let a = s.next().map(Oid::from_str);
            let b = s.next().map(Oid::from_str);

            match (a, b) {
                (Some(Ok(a)), Some(Ok(b))) => {
                    map.insert(a, Some(b));
                }
                (Some(Ok(a)), None) => {
                    map.insert(a, None);
                }
                _ => continue, // Ignore the parsing error.
            };
        }

        Ok(OidMap { map: map })
    }

    /// Writes this OidMap to a file.
    pub fn write<W: io::Write>(&self, f: &mut W) -> io::Result<()> {
        for (k, v) in &self.map {
            write!(f, "{}", k)?;

            if let Some(v) = *v {
                write!(f, " {}\n", v)?;
            }
        }

        Ok(())
    }

    pub fn get(&self, k: &Oid) -> Option<&Option<Oid>> {
        self.map.get(k)
    }

    /// Resolves an OID through multiple indirections.
    pub fn resolve(&self, k: &Oid) -> Option<&Option<Oid>> {
        match self.map.get(k) {
            Some(value) => match *value {
                Some(oid) => {
                    if k == &oid {
                        Some(value)
                    } else if self.map.contains_key(&oid) {
                        self.resolve(&oid)
                    } else {
                        Some(value)
                    }
                }
                None => Some(value),
            },
            None => None,
        }
    }

    pub fn insert(&mut self, k: Oid, v: Option<Oid>) -> Option<Option<Oid>> {
        self.map.insert(k, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Oid;

    #[test]
    fn test_resolve() {
        let mut map = OidMap::new();

        let a =
            Oid::from_str("0000000000000000000000000000000000000000").unwrap();
        let b =
            Oid::from_str("0000000000000000000000000000000000000001").unwrap();
        let c =
            Oid::from_str("0000000000000000000000000000000000000002").unwrap();

        map.insert(a, Some(b));
        assert_eq!(map.resolve(&a), Some(&Some(b)));
        assert_eq!(map.resolve(&b), None);

        map.insert(b, Some(c));
        assert_eq!(map.resolve(&a), Some(&Some(c)));
        assert_eq!(map.resolve(&b), Some(&Some(c)));

        map.insert(c, None);
        assert_eq!(map.resolve(&a), Some(&None));
        assert_eq!(map.resolve(&b), Some(&None));
        assert_eq!(map.resolve(&c), Some(&None));
    }
}
