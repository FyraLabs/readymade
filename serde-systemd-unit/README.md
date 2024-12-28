# Systemd Unit format parser

This crate provides a parser for systemd unit files, and implements `serde::Deserialize` for it.

## Example

```rust
use serde_systemd_unit::SystemdIni;

let unit = r#"
[Unit]
Description=Test unit

[Service]

ExecStart=/bin/true

[Install]
WantedBy=multi-user.target

"#;
let mut ini = SystemdIni::new();
ini.parse(unit).unwrap();

assert_eq!(ini["Unit"]["Description"].as_str(), "Test unit");
```
