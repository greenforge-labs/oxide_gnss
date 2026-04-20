# Contributing

`oxide_gnss` is released as-is from internal use. Only occasional feature development is planned. Bug-fix PRs and forks are welcome, but responses may be slow.

## Build & test

```bash
colcon build --packages-up-to oxide_gnss_msgs oxide_gnss
source install/setup.bash

cd src/oxide_gnss
cargo test   --features ros2
cargo clippy --features ros2 -- -D warnings
cargo fmt --all -- --check
```

Or, if you have `just` installed, `just ci` runs all three checks.

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for workspace setup (ros2-rust, colcon-cargo).

## What's most likely to land

- Bug reports with reproduction steps
- Documentation fixes
- Support for additional UBX messages
- Tests for existing behaviour

## License

MIT. By contributing you agree your contribution is licensed under the same terms.
