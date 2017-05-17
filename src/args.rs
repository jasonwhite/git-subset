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

use clap::{App, Arg, ArgMatches};

use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub quiet: bool,
    pub force: bool,
    pub nomap: bool,
    pub repo: String,
    pub branch: String,
    pub filter_file: Option<String>,
    pub paths: Vec<PathBuf>,
    pub revspec: String,
}

impl Args {

    pub fn parse() -> Self {
        let matches = App::new(crate_name!())
            .version(crate_version!())
            .author(crate_authors!())
            .about(crate_description!())
            .args(&[
                Arg::with_name("quiet")
                    .help("Don't print as much progress.")
                    .long("quiet")
                    .short("q"),

                Arg::with_name("force")
                    .help("Overwrites the branch name if it exists.")
                    .long("force")
                    .short("f"),

                Arg::with_name("nomap")
                    .help("Does not use the saved map. Useful for profiling purposes.")
                    .long("nomap"),

                Arg::with_name("repo")
                    .help("Path to the repository. Defaults to the current directory.")
                    .takes_value(true)
                    .long("repo")
                    .short("r")
                    .default_value("."),

                Arg::with_name("filter-file")
                    .help("Path to the file containing paths to keep.")
                    .takes_value(true)
                    .long("filter-file"),

                Arg::with_name("path")
                    .help("Path to include. Can be specified multiple times.")
                    .takes_value(true)
                    .short("p")
                    .multiple(true)
                    .long("path"),

                Arg::with_name("revspec")
                    .help("The ref to filter from.")
                    .index(1)
                    .default_value("HEAD"),

                Arg::with_name("branch")
                    .help("Name of the branch to create on the rewritten commits.")
                    .takes_value(true)
                    .long("branch")
                    .short("b")
                    .required(true),
            ])
            .get_matches();

        Args::parse_matches(&matches)
    }

    fn parse_matches<'a>(matches: &ArgMatches<'a>) -> Self {
        Args {
            quiet: matches.is_present("quiet"),
            force: matches.is_present("force"),
            nomap: matches.is_present("nomap"),
            repo: matches.value_of("repo").unwrap().to_string(),
            branch: matches.value_of("branch").unwrap().to_string(),
            filter_file: matches.value_of("filter-file").map(|s| s.to_string()),
            revspec: matches.value_of("revspec").unwrap().to_string(),
            paths: match matches.values_of("path") {
                None => vec![],
                Some(values) => values.map(|s| PathBuf::from(s)).collect(),
            },
        }
    }
}
