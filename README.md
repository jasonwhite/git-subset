# git-subset

[![Build Status](https://travis-ci.org/jasonwhite/git-subset.svg?branch=master)](https://travis-ci.org/jasonwhite/git-subset)
[![Build status](https://ci.appveyor.com/api/projects/status/6j0ifjf3521w0t4w/branch/master?svg=true)](https://ci.appveyor.com/project/jasonwhite/git-subset/branch/master)

This is a tool to filter a Git repository for a whitelist of files and folders.
It has a more narrow scope than `git filter-branch` but is *significantly*
faster.

## Why?

This tool was created to limit access to a very large proprietary codebase (for
security reasons). That is, the `master` branch in repo `A` is filtered into
repo `B` where those with access to `B` only see a small subset of the real
repo `A`.

## Usage

    $ git-subset --help
    USAGE:
        git-subset [FLAGS] [OPTIONS] --branch <branch> [--] [revspec]

    FLAGS:
        -f, --force      Overwrites the branch name if it exists.
        -h, --help       Prints help information
            --nomap      Does not use the saved map. Useful for profiling purposes.
        -q, --quiet      Don't print as much progress.
        -V, --version    Prints version information

    OPTIONS:
        -b, --branch <branch>              Name of the branch to create on the rewritten commits.
            --filter-file <filter-file>    Path to the file containing paths to keep.
        -p, --path <path>...               Path to include. Can be specified multiple times.
        -r, --repo <repo>                  Path to the repository. Defaults to the current directory. [default: .]

    ARGS:
        <revspec>    The ref to filter from. [default: HEAD]

## Contrived Example

Suppose we want to create a subset of the Linux source tree and we have a list
of the files and folders we want to keep:

    $ cat linux.filter
    README
    COPYING
    Makefile
    include/
    fs/btrfs/

Now, clone the Linux kernel (or another repository that isn't so *YUGE*):

    $ git clone https://github.com/torvalds/linux.git

*10 hours later...*

    $ cd linux

Now, to filter out everything that isn't listed in `linux.filter`. (Brace
yourself for awesome speed!)

    $ git-subset --filter-file ../linux.filter --branch new-master
    Getting list of commits...
    Rewriting 4327da054142f4dbf74615918b71441d95025bad (678123/678123) - 100%
    Branch 'new-master' created.

On my test machine with an SSD, this churned through 678,123 commits in about
**3 minutes**. This is *far* faster than `git filter-branch`. Let's just say
that it probably took less time to write this tool than it would have for `git
filter-branch` to finish running.

Running it again after pulling down the latest changes...

    $ git pull
    $ git-subset --filter-file ../linux.filter --branch new-master --force

...took about **20 seconds** because the mapping of old commit hashes to new
commit hashes has been cached from the previous run (use `--nomap` to disable
this).

Now, the new commit history is in the `new-master` branch and it contains only
the history for the list of files and folders we specified:

    $ git ls-tree new-master
    100644 blob ca442d313d86dc67e0a2e5d584b465bd382cbf5c    COPYING
    100644 blob 470bd4d9513ac42eb164cb4513300966a726fa37    Makefile
    100644 blob b2ba4aaa3a71046653599aa0b3798b211a2c0d30    README
    040000 tree 248cb042ad04f4b6d90a876b7ca35d1617de1e46    fs
    040000 tree d3ba01442799c0b5169cc3daeb6ab7da150f47dd    include

`new-master` can then be pushed to a new repository that contains only the
history of the files and folders we want.

## Related Tools

 * The [BFG Repo Cleaner](https://github.com/rtyley/bfg-repo-cleaner).

   This is similar, but doesn't have the same functionality. The BFG is more
   useful for filtering out specific files than it is for whitelisting file
   paths.

 * [GitRocketFilter](https://github.com/xoofx/GitRocketFilter/).

   GitRocketFilter can do everything this tool can do (plus more!), but more
   slowly. While GitRocketFilter is more generic, `git-subset` is designed for
   one very specific use-case: creating a subset of a repository. As a result,
   `git-subset` can be very aggressive about avoiding work.

## License

[MIT License](/LICENSE)

## Thanks

This tool was developed for internal use at [Environmental Systems Research
Institute](http://www.esri.com/) (Esri) who have graciously allowed me to retain
the copyright and publish it as open source software.
