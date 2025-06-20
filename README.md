# OpenJudge CLI

A command line tool for https://openjudge.cn

## Build

Install binary to PATH with

```sh
cargo install --path .
```

You may also need to add `$HOME/.cargo/bin` to PATH.

Default build product does not include sixel support.

> [!IMPORTANT]
>
> You may encounter various problems when building with sixel support,
> if possible, use Kitty Image Protocol or iTerm inline image instead.

To enable sixel support, build with `--features sixel`. You need
to have libjpeg installed.

You can run `source configure_libjpeg.sh`, tested on macOS, which
clears build cache and configures env variables for you.

If you need to configure it yourself, you need to remove the build
cache under `target/debug/build/sixel-sys-*`

## About Terminal Emulators Support

- For syntex highlighting to work, your terminal emulator must support
  24-bit color
- For graphics protocol support:
  - Auto detection now supports VSCode, Ghostty, Kitty, iTerm
  - For other terminal emulators, graphics are by default disabled
  - Consult your emulators documentations to configure graphics protocol
    accordingly

## Usage

Refer `oj --help`

## Troubleshooting

### DBus related errors

On linux systems, you will need to install packages which provides
`org.freedesktop.secrets` support in order for keyring services to
work. [`gnome-keyring`](https://wiki.archlinux.org/title/GNOME/Keyring)
was tested to work.

### Couldn't access platform secure storage

You need a default keyring store. Create one and set it as default.