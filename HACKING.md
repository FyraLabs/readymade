# Hacking Readymade

Readymade is an OS installer written in Rust, and currently a frontend for [Albius](https://github.com/Vanilla-OS/Albius).
There are plans to create a dedicated backend for Readymade in the future for better integration with the installer.

The UI code is written in [Relm](https://relm4.org/), a GTK4 UI framework for Rust inspired by Elm.

## Dedicated backend

The new dedicated backend for Readymade should be written in Rust, and should be able to handle the following tasks:

- Declarative(?) disk partitioning and generation of actions to be fed to UDisks2
- UDisks2 integration for disk partitioning and formatting (and possibly LVM and BTRFS support)
- Smart detection for Chromebook devices and other devices that require special handling (so that we can install extra Submarine bootloader payloads when required)
- Automatic systemd mountpoint hints using GPT partition labels/flags

### If you're gonna do this in Rust, why not just use [distinst](https://github.com/pop-os/distinst)?

We have considered using distinst, but we have decided to write our own installer for the following reasons:

- distinst is being deprecated in favor of distinst2, which runs a dedicated D-Bus backend service for the installer.
- Code from distinst is heavily tied to Debian/Ubuntu and Pop!\_OS, and it is not easily portable to other distributions.
- Regarding above point, System76's APIs are not very well documented and maintained, and requires a lot of hacking to even get working (see the `old` branch of this repository for an example of how we tried to get it working
  )
- And because of the above points, there is only one distribution that uses distinst, and that is elementary OS. We want to make Readymade a worthy alternative to Anaconda, which means improving upon the installer experience for for RPM distributions as well.

## Building

To build Readymade, you need to have the following dependencies installed:

- GTK4
- libhelium
- libgnome-desktop4
- GNU Gettext (for l10n)
- Rust

To build Readymade, simply run:

```sh
cargo build --release # Release build, omit --release for a debug build with symbols and assertions
```

The build should be successful if all dependencies are installed.

## Running

There are however more runtime dependencies required to actually run Readymade, which includes:

- Submarine (place your disk image in `/usr/share/submarine`)
- Albius (`/usr/bin/albius`)
- `pkexec` (For escalating the process as root)

You can simply run Readymade by executing the binary. It should be located in `target/release/readymade`.

There are also extra PolicyKit rules to skip password prompts for `pkexec` to escalate the process as root.
Copy the `com.fyralabs.pkexec.readymade.policy` file to `/usr/share/polkit-1/actions/` and restart the PolicyKit service.

## Debugging

Readymade currently forces all logging to be at the `trace` level for debugging purposes. This is the most verbose level available in tracing.
Readymade logs to stderr and to a file called `/tmp/readymade.log`. The file logger is powered by `tracing-appender`.

Currently Readymade only supports Chromebook installations, it is recommended you run Readymade on a Chromebook device to test the installer.

Readymade checks for Dracut's default `live-base` (in `/dev/mapper/live-base`) logical volume for the base filesystem to mount and copy from. This is usually generated with Dracut's live module. It then tries to mount the base filesystem from the logical volume and use the files from there as the source for the installer.

You can however override this by setting the environment variable `REPART_COPY_SOURCE` to the path of the base filesystem to copy from. This makes use of systemd 255's new relative repart source feature. Note that you may need to set this while running Readymade with `sudo` to ensure that the environment variable is passed to the process, instead of the default behavior of Readymade restarting itself as root using `pkexec` which does not pass any environment variables.

```sh
sudo REPART_COPY_SOURCE=/mnt/rootfs readymade
```

## Localization

You can translate Readymade to your language by going to the [Fyra Labs Weblate](https://weblate.fyralabs.com/projects/tauOS/readymade/) page and translating the strings there.

## Contributing

Before pushing your changes, please make sure to run `cargo fmt` to format your code according to the Rust style guidelines provided in `rustfmt.toml`.

You should also run `cargo clippy` to check for any potential style issues or bugs in your code.

And if possible, write tests for your code, and run `cargo test` to ensure that your code works as expected.

---

# Tasks

## Generating pot file

```
cargo install xtr
xtr src/main.rs -o po/readymade.pot --package-name Readymade --package-version 0.1.0
```

## Installing po files

(Taking Japanese as an example)

```
msgfmt po/ja.po -o /usr/share/locale/ja/LC_MESSAGES/com.fyralabs.Readymade.mo
```
