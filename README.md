# OpenJudge CLI

A command line tool for https://openjudge.cn

## Build

```sh
cargo install --path .
```

Default build product does not include sixel support.

To enable sixel support, build with `--features sixel`. You need
to have libjpeg installed.

You can run `source configure_libjpeg.sh`, tested on macOS, which
clears build cache and configures env variables for you.

If you need to configure it yourself, you need to remove the build
cache under `target/debug/build/sixel-sys-*`

## Usage

Refer `oj --help`