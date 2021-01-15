// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::env;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process;
use std::time;

use getopts::Options;

fn usage(progname: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [options] [file]", progname);
    eprint!("{}", opts.usage(&brief));
}

fn run() -> i32 {
    let args: Vec<String> = env::args().collect();
    let progname = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("i", "id", "Window to capture", "ID");
    opts.optopt("g", "geometry", "Area to capture", "WxH+X+Y");
    opts.optopt("f", "format", "Output format", "png/pam");
    opts.optflag("h", "help", "Print help and exit");
    opts.optflag("v", "version", "Print version and exit");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            usage(&progname, opts);
            return 1;
        }
    };

    if matches.opt_present("h") {
        usage(&progname, opts);
        return 0;
    }

    // One loose argument allowed (file name)
    if matches.free.len() > 1 {
        eprintln!("Too many arguments");
        usage(&progname, opts);
        return 1;
    }

    if matches.opt_present("v") {
        eprintln!(
            "shotgun {}",
            option_env!("GIT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
        );
        return 0;
    }

    let output_ext = matches
        .opt_str("f")
        .unwrap_or("png".to_string())
        .to_lowercase();
    let output_format = match output_ext.as_ref() {
        "png" => image::ImageOutputFormat::Png,
        "pam" => image::ImageOutputFormat::Pnm(image::pnm::PNMSubtype::ArbitraryMap),
        _ => {
            eprintln!("Invalid image format specified");
            return 1;
        }
    };

    let window_id = matches
        .opt_str("i")
        .map(|id| match parse_int::parse::<u64>(&id) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Window ID {:?} is not a valid integer\nAccepted values are decimal, hex (0x*), octal (0o*) and binary (0b*)", id);
                return 2;
            }
        });

    let image = shotgun::capture(window_id, matches.opt_str("g"))
        .map_err(|e| eprintln!("{}", e))
        .expect("unable to capture");

    let ts_path = {
        let now = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => 0,
        };
        format!("{}.{}", now, output_ext)
    };
    let path = match matches.free.get(0) {
        Some(p) => p,
        None => {
            eprintln!("No output specified, defaulting to {}", ts_path);
            ts_path.as_str()
        }
    };

    if path == "-" {
        image
            .write_to(&mut io::stdout(), output_format)
            .expect("Writing to stdout failed");
    } else {
        match File::create(&Path::new(&path)) {
            Ok(mut f) => image
                .write_to(&mut f, output_format)
                .expect("Writing to file failed"),
            Err(e) => {
                eprintln!("Failed to create {}: {}", path, e);
                return 1;
            }
        }
    }

    0
}

fn main() {
    process::exit(run());
}
