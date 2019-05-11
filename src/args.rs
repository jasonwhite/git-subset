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
use std::path::PathBuf;

use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Args {
    /// Don't print as much progress.
    #[structopt(long = "quiet", short = "q")]
    pub quiet: bool,

    /// Overwrites the branch name if it exists.
    #[structopt(long = "force", short = "f")]
    pub force: bool,

    /// Does not use the saved map. Useful for benchmarking purposes.
    #[structopt(long = "nomap")]
    pub nomap: bool,

    /// Path to the repository. Defaults to the current directory.
    #[structopt(long = "repo", short = "r", default_value = ".")]
    pub repo: PathBuf,

    /// Name of the branch to create on the rewritten commits.
    #[structopt(long = "branch", short = "b")]
    pub branch: String,

    /// Path to the file containing paths to keep.
    #[structopt(long = "filter-file")]
    pub filter_file: Option<PathBuf>,

    /// Path to include. Can be specified multiple times.
    #[structopt(long = "path", short = "p")]
    pub paths: Vec<PathBuf>,

    /// The ref to filter from.
    #[structopt(default_value = "HEAD")]
    pub revspec: String,
}
