[![askii](askii.png)](https://github.com/nytopop/askii)

A tool for drawing ASCII diagrams.

# Installation
Install a [binary package](https://github.com/nytopop/askii/releases), or use `cargo install askii` to compile the latest source from [crates.io](https://crates.io/crates/askii).

# Compilation
Use `cargo`. The binary dynamically links against `libncursesw.so.6`, so make sure it is available.

Alternatively, the [`Makefile`](Makefile) can be used to build a binary and DEB / RPM packages.

```
cd askii && make
```

The produced artifacts will be located in `askii/dist`.

It requires:

- [GNU Make](https://www.gnu.org/software/make/)
- [jq](https://stedolan.github.io/jq/)
- [fpm](https://github.com/jordansissel/fpm)

# License
Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
