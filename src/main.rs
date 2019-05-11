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
use structopt::StructOpt;

mod args;
mod filter;
mod map;

use std::cmp;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::process;
use std::str;

use crate::args::Args;
use crate::filter::{filter_tree, Filter};
use crate::map::OidMap;

/// Returns `true` if the given commit is considered empty. A commit is empty if
/// its tree is the same as all of its parent's trees, or if it has no parents
/// and the tree itself is empty.
fn is_empty_commit(commit: &git2::Commit<'_>, empty_tree: &git2::Oid) -> bool {
    let mut parents = 0;
    let mut same = 0;

    for parent in commit.parents() {
        if commit.tree_id() == parent.tree_id() {
            same += 1;
        }

        parents += 1;
    }

    if parents > 0 {
        parents == same
    } else {
        commit.tree_id() == *empty_tree
    }
}

/// Rewrites the trees of the commits starting with the HEAD commit. Returns the
/// new tip commit OID.
fn process_commits(
    repo: &git2::Repository,
    revspec: &git2::Revspec<'_>,
    map: &mut OidMap,
    filter: &Filter,
    quiet: bool,
) -> Result<Option<git2::Oid>, git2::Error> {
    let mut commits = repo.revwalk()?;
    commits.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE);

    match (revspec.from(), revspec.to()) {
        (Some(from), Some(to)) => {
            commits.hide(from.id())?;
            commits.push(to.id())?;
        }
        (Some(from), None) => {
            commits.push(from.id())?;
        }
        _ => {
            // Unsure if this branch can ever get taken.
            panic!("Invalid revspec");
        }
    };

    // An empty tree OID
    let empty_tree =
        git2::Oid::from_str("4b825dc642cb6eb9a060e54bf8d69288fbee4904")?;

    // Store the last commit to be processed. This is returned so that we can
    // create a branch on it.
    let mut last = None;

    if !quiet {
        println!("Getting list of commits...");
    }

    // Collect commits into an array so that we can print progress.
    let commits = commits.collect::<Result<Vec<_>, git2::Error>>()?;

    // We want to (at most) print the status for each percentage point.
    // Printing the status too often can slow down the program.
    let status_step = cmp::max(commits.len() / 100, 1);

    for (i, id) in commits.iter().enumerate() {
        let id = id.clone();

        if !quiet && i % status_step == 0 {
            print!(
                "\rRewriting {} ({}/{}) - {:3.0}%",
                id,
                i + 1,
                commits.len(),
                ((i + 1) as f32) / (commits.len() as f32) * 100.0
            );
            io::stdout().flush().unwrap();
        }

        let commit =
            repo.find_commit(process_commit(repo, map, id, filter)?)?;

        // Store mapping between the old commit and new commit. This is used to
        // remap parent commits.
        map.insert(id, Some(commit.id()));

        // Discard this commit if its tree is the same as all of its parent's
        // trees. There may be multiple levels of indirection if several commits
        // in a row are discarded.
        if is_empty_commit(&commit, &empty_tree) {
            // Map it to its parent so that subsequent commits resolve to the
            // parent of this commit instead. It doesn't matter which parent we
            // choose, since they must all be identical.
            //
            // *Note*: Even though this commit has already been created, it is
            // left behind as an unreferenced dangling commit to be garbage
            // collected.
            if let Some(parent) = commit.parents().next() {
                map.insert(commit.id(), Some(parent.id()));
            } else {
                // If this is a root commit, we need to make the next commit
                // become the root commit. Thus, we mark this commit as
                // discarded.
                map.insert(commit.id(), None);
            }
        } else {
            // If the final commit is empty, don't return it.
            last = Some(commit.id());
        }
    }

    if let Some(commit) = last {
        // Print the final status.
        println!(
            "\rRewriting {} ({}/{}) - 100%",
            commit,
            commits.len(),
            commits.len()
        );
    }

    Ok(last)
}

/// Rewrites a single commit. Returns the new OID for the commit.
fn process_commit(
    repo: &git2::Repository,
    map: &mut OidMap,
    id: git2::Oid,
    filter: &Filter,
) -> Result<git2::Oid, git2::Error> {
    // Don't bother if it has already been done.
    if let Some(&Some(newid)) = map.get(&id) {
        return Ok(newid);
    }

    let commit = repo.find_commit(id)?;

    let tree = commit.tree()?;

    let newtree = filter_tree(repo, map, filter, &tree)?;

    // Get the new parent OIDs.
    let parents: Vec<_> = commit
        .parent_ids()
        .filter_map(|p| match map.resolve(&p) {
            Some(&Some(p)) => repo.find_commit(p).ok(),
            _ => None,
        })
        .collect();

    let author = commit.author();
    let committer = commit.committer();

    repo.commit(
        None,
        &author,
        &committer,
        unsafe { str::from_utf8_unchecked(commit.message_bytes()) },
        &repo.find_tree(newtree)?,
        &parents.iter().collect::<Vec<_>>(), // Convert from &[T] to &[&T].
    )
}

/// Creates a subset of a repository.
fn repo_subset(
    repo: &git2::Repository,
    map: &mut OidMap,
    filter: &Filter,
    revspec: &str,
    branch: &str,
    force: bool,
    quiet: bool,
) -> Result<bool, git2::Error> {
    let revspec = repo.revparse(revspec)?;

    match process_commits(repo, &revspec, map, filter, quiet)? {
        Some(oid) => {
            // Create the branch based on the last processed commit.
            let commit = repo.find_commit(oid)?;
            repo.branch(branch, &commit, force)?;
            Ok(true)
        }
        None => {
            // No commits and therefore no branch to create.
            Ok(false)
        }
    }
}

/// Entry point for the program.
///
/// The program works in the following way:
///  1. Traverse the graph in reverse topological order.
///  2. For each commit, rewrite its tree such that the tree only includes the
///     specified files and directories.
///     * The tree rewrite must be cached to avoid unnecessary work. That is, an
///       OID mapping must be stored. For most commits, most of the tree is
///       unchanged, so this provides a significant speedup.
///     * The mapping can be persisted inside the .git directory as long as the
///       filter does not change. Thus, we store the mapping named by the hash
///       of the tree filter. This can be useful for incrementally rewriting the
///       tree as new commits are added.
///     * If a commit is empty it is discarded.
///     * The parent commit must also be fixed for each commit (except for the
///       root commit).
///  3. Create a branch on the new tip commit.
fn main() {
    let args = Args::from_args();

    let repo = match git2::Repository::open(args.repo) {
        Ok(repo) => repo,
        Err(err) => {
            println!("Error: Failed to open repository: {}", err);
            process::exit(1);
        }
    };

    let mut filter = match args.filter_file {
        Some(path) => match Filter::from_file(&path) {
            Ok(filter) => filter,
            Err(err) => {
                println!(
                    "Error: Failed to load filter file '{}': {}",
                    path.display(),
                    err
                );
                process::exit(1);
            }
        },
        None => Filter::new(),
    };

    for path in &args.paths {
        filter.insert(path);
    }

    if filter.is_empty() {
        println!(
            "Error: Please specify paths to include with either \
             `--filter-file` or `--path`."
        );
        process::exit(1);
    }

    // Name of the map file.
    let map_name = {
        // The map path is derived from the hash of the filter so that we don't
        // use an invalid object mapping for subsequent runs.
        let mut hasher = DefaultHasher::new();
        filter.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    };

    let mut map = if args.nomap {
        OidMap::new()
    } else {
        match OidMap::from_repo(&repo, &map_name) {
            Ok(map) => map,
            Err(err) => {
                println!("Error: Failed to load object map: {}", err);
                process::exit(1);
            }
        }
    };

    match repo_subset(
        &repo,
        &mut map,
        &filter,
        &args.revspec,
        &args.branch,
        args.force,
        args.quiet,
    ) {
        Ok(true) => {
            println!("Branch '{}' created.", args.branch);
        }
        Ok(false) => {
            // FIXME: Create an orphaned branch instead?
            println!(
                "Error: Filtering only produced empty commits. No branch \
                 created."
            );
            process::exit(1);
        }
        Err(err) => {
            println!("Error: Failed to create repository subset: {}", err);
            process::exit(1);
        }
    };

    // Save the mapping for super fast filtering next time.
    if let Err(err) = map.write_repo(&repo, &map_name) {
        println!("Error: Failed to write object map: {}", err);
        process::exit(1);
    }
}
