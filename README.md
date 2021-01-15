# shotgun

A minimal screenshot utility for X11. Shotgun was written to replace
[maim](https://github.com/naelstrof/maim) in my workflow.

Features:
- Exports PNG screenshots to file or stdout
- Masks off-screen areas on multi-head setups
- Supports selections by window ID and geometry
- *On average, shotgun is more than twice as fast as maim*

## Usage

```
Usage: shotgun [options] [file]

Options:
    -i, --id ID         Window to capture
    -g, --geometry WxH+X+Y
                        Area to capture
    -f, --format png/pam
                        Output format
    -h, --help          Print help and exit
    -v, --version       Print version and exit
```

## Examples

#### To use with hacksaw: take a screenshot and copy to clipboard
```sh
#!/bin/sh -e

selection=$(hacksaw -f "-i %i -g %g")
shotgun $selection - | xclip -t 'image/png' -selection clipboard
```

#### To use with slop (as a replacement for `maim -s`):
```sh
#!/bin/sh -e

sel=$(slop -f "-i %i -g %g")
shotgun $sel "$1"
```

## shotgun vs maim

- Only PNG and [PAM](#going-faster) are supported
- Does not attempt to wrap slop
- No cursor blending
- Defaults to a time-stamped file instead of dumping raw PNG data into your
  terminal (use `-` as the file name if you want to pipe output to something
  else)
- Most command-line flags were omitted
- The XShape extension is not supported
- shotgun is written in Rust, maim in C++
- The code base is kept as small and simple as possible (as much as Rust
  permits)

There are several reasons for omitting these features:
- Features that can be replaced trivially by external programs and wrapper
  scripts:
  - Use ImageMagick's `convert` and [shotgun's `-f pam`](#going-faster) for JPEG output
  - slop output is easy to process in a shell script
  - Use `sleep` instead of `-d`, since slop has to be called separately, this
    flag is not necessary
  - `-x` shouldn't even exist in the first place, set `$DISPLAY` instead
- I never use cursor blending, and I know that most users do not actually care
  for it
- `-w` (geometry relative to another window) is difficult to use and hardly
  useful, instead, shotgun always interprets the input geometry relative to the
  root window (maim's default is the captured window itself)
- There is rarely a reason to take a screenshot of an XShape window, most of
  them are special like slop's selection window or keynav's crosshair.
  Supporting XShape properly could add a significant amount of overhead, both in
  code length and performance, which are not desirable.

## Performance

I've claimed that shotgun is twice as fast as maim, here's some supporting
evidence (using [`hyperfine`](https://github.com/sharkdp/hyperfine)):

```
$ xrandr --fb 3840x2160
$ hyperfine --warmup 15 --min-runs 50 \
>     'maim > /dev/null' \
>     'shotgun - > /dev/null'
Benchmark #1: maim > /dev/null
  Time (mean ± σ):     629.3 ms ±   3.7 ms    [User: 570.7 ms, System: 52.1 ms]
  Range (min … max):   624.8 ms … 646.7 ms    50 runs

Benchmark #2: shotgun - > /dev/null
  Time (mean ± σ):     293.0 ms ±   3.2 ms    [User: 239.6 ms, System: 52.9 ms]
  Range (min … max):   287.8 ms … 298.2 ms    50 runs

Summary
  'shotgun - > /dev/null' ran
    2.15 ± 0.03 times faster than 'maim > /dev/null'
```

Further profiling has shown that the bottleneck in shotgun lies fully within the
PNG encoder.

### Going faster

The PNG encoder bottleneck can be avoided by using `-f pam`. This sets the output format to
[Netpbm PAM](https://en.wikipedia.org/wiki/Netpbm#PAM_graphics_format) - an uncompressed binary image format.

By using an uncompressed format both encoding and decoding performance is improved:

#### Encoding

```
$ hyperfine --warmup 15 --min-runs 50 \
>     'shotgun -f png - > /dev/null' \
>     'shotgun -f pam - > /dev/null'
Benchmark #1: shotgun -f png - > /dev/null
  Time (mean ± σ):     294.5 ms ±   3.3 ms    [User: 240.0 ms, System: 54.1 ms]
  Range (min … max):   289.2 ms … 301.4 ms    50 runs

Benchmark #2: shotgun -f pam - > /dev/null
  Time (mean ± σ):     116.8 ms ±   2.8 ms    [User: 62.5 ms, System: 53.7 ms]
  Range (min … max):   113.8 ms … 122.7 ms    50 runs

Summary
  'shotgun -f pam - > /dev/null' ran
    2.52 ± 0.07 times faster than 'shotgun -f png - > /dev/null'
```

#### Decoding (using ImageMagick to convert to jpg)

```
$ hyperfine --warmup 15 --min-runs 50 \
>     'shotgun -f png - | convert - jpg:- > /dev/null' \
>     'shotgun -f pam - | convert - jpg:- > /dev/null'
Benchmark #1: shotgun -f png - | convert - jpg:- > /dev/null
  Time (mean ± σ):     600.7 ms ±   5.8 ms    [User: 506.4 ms, System: 96.5 ms]
  Range (min … max):   594.9 ms … 620.7 ms    50 runs

Benchmark #2: shotgun -f pam - | convert - jpg:- > /dev/null
  Time (mean ± σ):     350.4 ms ±   3.9 ms    [User: 217.0 ms, System: 139.3 ms]
  Range (min … max):   345.5 ms … 367.8 ms    50 runs

Summary
  'shotgun -f pam - | convert - jpg:- > /dev/null' ran
    1.71 ± 0.03 times faster than 'shotgun -f png - | convert - jpg:- > /dev/null'
```

## Installation

- From source:
  - install a recent Rust toolchain, libx11 and libxrandr (depending on your
    distribution, you may need to install development headers separately)
  - clone this repository and run `cargo install --features="cli" --path .`
  - or install from [crates.io](https://crates.io/crates/shotgun):
    `cargo install --features="cli" shotgun`
- [Arch Linux](https://www.archlinux.org/packages/?name=shotgun): `pacman -S shotgun`
- [Nix/NixOS](https://github.com/NixOS/nixpkgs-channels/blob/nixos-unstable/pkgs/tools/graphics/shotgun/default.nix): `nixpkgs.shotgun`
- Other distros: make a pull request to add your package or build script!

### Running from Source

```
cargo run --features="cli" --release
```
