# netstat

A terminal UI network statistics viewer built with Rust and [ratatui](https://github.com/ratatui/ratatui).

## Features

- **Interfaces tab** -- shows all network interfaces with RX/TX bytes,
  packets, errors, drops, and live transfer rates. Includes a bar chart
  of total traffic per interface.
- **Connections tab** -- lists active TCP and UDP sockets with local/remote
  addresses, connection state, UID, and inode. Color-coded TCP state
  summary. Filter by protocol with `f`.
- **Bandwidth tab** -- real-time sparkline graphs of RX/TX rates per
  interface with a rolling 60-sample history.

Data is read from `/proc/net` via the `procfs` crate (Linux only).

## Requirements

- Rust 1.85+ (edition 2024)
- Linux (reads `/proc/net/*`)

## Build & Run

```sh
cargo build --release
./target/release/netstat
```

Or simply:

```sh
cargo run
```

## Keybindings

| Key               | Action               |
|-------------------|----------------------|
| `Tab` / `Right`   | Next tab             |
| `Shift+Tab`/`Left`| Previous tab         |
| `1` `2` `3`       | Jump to tab          |
| `j` / `Down`      | Scroll down          |
| `k` / `Up`        | Scroll up            |
| `f`               | Cycle connection filter (All/TCP/UDP) |
| `q` / `Esc`       | Quit                 |

## License

Apache-2.0. See [LICENSE](LICENSE).
