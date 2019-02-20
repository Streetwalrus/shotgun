// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process;

extern crate getopts;
use getopts::Options;
extern crate image;
use image::GenericImage;
use image::Pixel;
use image::RgbaImage;
use image::Rgba;
extern crate libc;
extern crate time;
extern crate x11;
use x11::xlib;

mod util;
mod xwrap;
use xwrap::Display;

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
        eprintln!("shotgun {}", option_env!("GIT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")));
        return 0;
    }

    let display = match Display::open(None) {
        Some(d) => d,
        None => {
            eprintln!("Failed to open display");
            return 1;
        }
    };
    let root = display.get_default_root();

    let window = match matches.opt_str("i") {
        Some(s) => match s.parse::<xlib::Window>() {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Window ID is not a valid integer");
                return 1;
            },
        },
        None => root,
    };

    let output_ext = matches.opt_str("f").unwrap_or("png".to_string()).to_lowercase();
    let output_format = match output_ext.as_ref() {
        "png" => image::ImageOutputFormat::PNG,
        "pam" => image::ImageOutputFormat::PNM(image::pnm::PNMSubtype::ArbitraryMap),
        _ => {
            eprintln!("Invalid image format specified");
            return 1;
        }
    };

    let window_rect = display.get_window_rect(window);
    let sel = match matches.opt_str("g") {
        Some(s) => match xwrap::parse_geometry(CString::new(s).expect("Failed to convert CString"))
                         .intersection(window_rect) {
            Some(sel) => util::Rect {
                // Selection is relative to the root window (whole screen)
                x: sel.x - window_rect.x,
                y: sel.y - window_rect.y,
                w: sel.w,
                h: sel.h,
            },
            None => {
                eprintln!("Invalid geometry");
                return 1;
            },
        },
        None => util::Rect {
            x: 0,
            y: 0,
            w: window_rect.w,
            h: window_rect.h,
        },
    };

    let image = match display.get_image(window, sel, xwrap::ALL_PLANES, xlib::ZPixmap) {
        Some(i) => i,
        None => {
            eprintln!("Failed to get image from X");
            return 1;
        },
    };

    let mut image = match image.into_image_buffer() {
        Some(i) => image::ImageRgba8(i),
        None => {
            eprintln!("Failed to convert captured framebuffer, only 24/32 \
                      bit (A)RGB8 is supported");
            return 1;
        }
    };

    // When capturing the root window, attempt to mask the off-screen areas
    if window == root {
        match display.get_screen_rects(root) {
            Some(screens) => {
                let screens: Vec<util::Rect> =
                    screens.filter_map(|s| s.intersection(sel)).collect();

                // No point in masking if we're only capturing one screen
                if screens.len() > 1 {
                    let mut masked = RgbaImage::from_pixel(sel.w as u32, sel.h as u32,
                                                           Rgba::from_channels(0, 0, 0, 0));

                    for screen in screens {
                        // Subimage is relative to the captured area
                        let sub = util::Rect {
                            x: screen.x - sel.x,
                            y: screen.y - sel.y,
                            w: screen.w,
                            h: screen.h,
                        };

                        let mut sub_src = image.sub_image(sub.x as u32, sub.y as u32,
                                                          sub.w as u32, sub.h as u32);
                        masked.copy_from(&mut sub_src, sub.x as u32, sub.y as u32);
                    }

                    image = image::ImageRgba8(masked);
                }
            },
            None => {
                eprintln!("Failed to enumerate screens, not masking");
            },
        }
    }

    let ts_path = format!("{}.{}", time::get_time().sec, output_ext);
    let path = match matches.free.get(0) {
        Some(p) => p,
        None => {
            eprintln!("No output specified, defaulting to {}", ts_path);
            ts_path.as_str()
        },
    };

    if path == "-" {
        image.write_to(&mut io::stdout(), output_format).expect("Writing to stdout failed");
    } else {
        match File::create(&Path::new(&path)) {
            Ok(mut f) => image.write_to(&mut f, output_format).expect("Writing to file failed"),
            Err(e) => {
                eprintln!("Failed to create {}: {}", path, e);
                return 1
            },
        }
    }

    0
}

fn main() {
    process::exit(run());
}
