# Vendored: librqbit-dualstack-sockets 0.7.0

Upstream: the crates.io release of the same name and version (Apache-2.0, `LICENSE`
unchanged). Patched into the workspace by `[patch.crates-io]` in the root `Cargo.toml`.

## Why

SPEC.md Phase 7 requires the torrent engine to **bind to a chosen network adapter**, so
that if a VPN drops, torrent traffic stops instead of falling back to the real
connection. librqbit exposes this as `SessionOptions::bind_device_name`, but this crate's
`BindDevice` returns `BindDeviceNotSupported` on Windows — the only platform this app
ships on.

## What changed

Only `src/bind_device.rs`, and one dependency in `Cargo.toml` (`windows-sys`, WinSock):

- `BindDevice::new_from_name` on Windows looks the adapter up by its friendly name
  ("Wi-Fi", "Ethernet", a VPN's adapter) through `network-interface`, which this crate
  already used, and keeps its interface index. An unknown name is an error.
- `BindDevice::bind_sref` on Windows sets `IP_UNICAST_IF` (index in network byte order)
  and, for IPv6 sockets, `IPV6_UNICAST_IF` (host order), which pin outgoing traffic to
  that interface. When the interface is gone, sends fail with "network unreachable".

Measured on 2026-11-05 (Wi-Fi up, a VPN adapter up, Ethernet disconnected): bound to the
VPN adapter a connection succeeded; bound to the disconnected adapter or to loopback it
failed with WSAENETUNREACH rather than leaving by Wi-Fi. Tested by
`crates/torrent/tests/binding.rs`, which fails with the socket option removed.

If upstream gains Windows support, drop the patch and this directory.
