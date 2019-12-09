<p align="center">
  <img src="askii.png" alt="logo">
</p>

TUI based ASCII diagram editor.

# Installation
Install a [binary release](https://github.com/nytopop/askii/releases), or use `cargo install askii` to compile the latest source release from [crates.io](https://crates.io/crates/askii).

To use askii on Windows or Mac, you'll need to compile it from source. Cross compilation of Rust programs that import C libraries (ncurses) is difficult at best, so binary releases are not provided here.

# Compilation
Use `cargo`. The binary links against `libncursesw.so.6`, so make sure it is available during compilation.

Alternatively, the [`Makefile`](Makefile) can be used to build a binary and deb / rpm / pacman packages.

```
cd askii && make
```

The produced artifacts will be located in `askii/dist`.

It requires:

- [GNU Make](https://www.gnu.org/software/make/)
- [jq](https://stedolan.github.io/jq/)
- [fpm](https://github.com/jordansissel/fpm)
- [libarchive](https://www.libarchive.org/)

# License
Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
