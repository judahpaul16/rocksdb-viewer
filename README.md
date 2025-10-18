# RocksDB Viewer

A terminal-based viewer for RocksDB databases with an interactive interface. This tool allows you to browse and search through RocksDB database contents in real-time.

## Features

- Interactive TUI (Terminal User Interface)
- Real-time database monitoring
- Key filtering capabilities
- Structured data visualization
- Auto-column sizing based on content
- Keyboard navigation

## Installation

```bash
cargo install --path .
```

## Usage

```bash
rocksdb-viewer --db-path /path/to/your/rocksdb
```

## Example Output

```
Filter by Key [Press 'q' to quit]
┌────────────────────────────────────────────────────┐
│                                                    │
└────────────────────────────────────────────────────┘

Transaction Records
┌──────────────────┬─────────────┬──────────┬────────────┐
│ Key              │ Timestamp   │ Value    │ Status     │
├──────────────────┼─────────────┼──────────┼────────────┤
│ tx:0x123...def   │ 1698512345  │ 1.5 ETH  │ Completed  │
│ tx:0x456...789   │ 1698512340  │ 0.5 ETH  │ Pending    │
│ tx:0x789...abc   │ 1698512335  │ 2.0 ETH  │ Failed     │
└──────────────────┴─────────────┴──────────┴────────────┘

Block Records
┌──────────────────┬─────────────┬──────────┬────────────┐
│ Key              │ Number      │ Hash     │ Size       │
├──────────────────┼─────────────┼──────────┼────────────┤
│ block:15234567   │ 15234567    │ 0xabc... │ 1.2 MB     │
│ block:15234566   │ 15234566    │ 0xdef... │ 0.8 MB     │
└──────────────────┴─────────────┴──────────┴────────────┘
```

## Navigation

- Up/Down arrows: Navigate records
- Left/Right arrows: Scroll horizontally
- PageUp/PageDown: Scroll pages
- Enter: Apply filter
- Backspace: Edit filter
- q or Esc: Quit

## Building from Source

```bash
git clone https://github.com/judahpaul16/rocksdb-viewer
cd rocksdb-viewer
cargo build --release
```

## Dependencies

- RocksDB
- Rust 1.88 or higher

## License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.