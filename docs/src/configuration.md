# Configuration

The goal of RMK's configuration system is to provide users an easy and accessible way to set up their keyboard (with or without Rust).

Apparently, a config file could be better for more people who don't know Rust, but we also want to keep some flexibility for customizing keyboard with Rust code.

There are two choices right now:

- [`cfg-toml`](https://github.com/jamesmunns/toml-cfg)
  - pros: 
    - a widely used lib
    - could overwrite default configs defined in RMK
    - easy to use 
  - cons:
    - need to add extra annotations to all config structs
    - some fields are not support
    - hard to expand to other types, accepts only numbers/strings in toml

- `build.rs`: Load the config in `build.rs`, then generate Rust code, which could be passed to RMK as config struct
  - pros:
    - Extendable, flexible, can do everything
    - No extra dependency
  - cons:
    - Need to distribute `build.rs`, users cannot use the lib without this file, which is not a good way generally
    - LOTS OF work
