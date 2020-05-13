<p align="center">
  <!-- project logo --!>
  <img src="askii.png" alt="logo"><br><br>
  <!-- crates.io version !-->
  <a href="https://crates.io/crates/askii">
    <img alt="Crates.io" src="https://img.shields.io/crates/v/askii?style=flat-square">
  </a>
  <!-- crates.io downloads --!>
  <a href="https://crates.io/crates/askii">
    <img alt="Crates.io" src="https://img.shields.io/crates/d/askii?style=flat-square">
  </a>
  <!-- github release downloads --!>
  <a href="https://github.com/nytopop/askii/releases">
    <img alt="GitHub All Releases" src="https://img.shields.io/github/downloads/nytopop/askii/total?style=flat-square">
  </a>
  <!-- crates.io license --!>
  <a href="./LICENSE-APACHE">
    <img alt="Apache-2.0 OR MIT" src="https://img.shields.io/crates/l/askii?style=flat-square">
  </a>
  <br><br>
  <a href="https://asciinema.org/a/329963" target="_blank"><img src="https://asciinema.org/a/329963.svg" /></a>
</p>

TUI based ASCII diagram editor.

# Installation
Install a [binary release](https://github.com/nytopop/askii/releases), or use `cargo install askii` to compile the latest source release from [crates.io](https://crates.io/crates/askii).

# Compilation
The binary links against a few X11 libs for clipboard functionality (on linux), so make sure they are available during compilation. On debian, they can be installed with:

```
apt install libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxau-dev libxdmcp-dev
```

Use `cargo` to compile. Alternatively, the [`Makefile`](Makefile) can be used to build a binary and deb / rpm / pacman packages.

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
