# RocksDB Viewer

A terminal-based viewer for RocksDB databases with an interactive interface.

## Features

- Interactive TUI (Terminal User Interface)
- Real-time database monitoring
- Key filtering capabilities
- Structured data visualization
- Auto-column sizing based on content
- Keyboard and mouse navigation
- Clear error messaging for database access issues

## Installation

```bash
cargo install --path .
```

## Usage

```bash
rocksdb-viewer --db-path /path/to/your/rocksdb
```

## Example Output

> _Format your RocksDB values as JSON for column mapping_

```
┌search:─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                                    │       
└────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌records─────────────────────────────────────────────────────────────────────────────────────────────┐
│┌album records────────────────────────────────────────────────────────────────────────────────────┐ │
││key                       artist           album               year    tracks    genre           │ │
││album:1234567:890123     Pink Floyd       Dark Side Moon     1973    10        Progressive       │ │
││album:1234567:890124     Led Zeppelin     IV                 1971    8         Rock              │ │
││album:1234567:890125     Miles Davis      Kind of Blue       1959    5         Jazz              │ │
││album:1234567:890126     Beatles          Abbey Road         1969    17        Rock              │ │
││album:1234567:890127     Queen            Night at Opera     1975    12        Rock              │ │
│└─────────────────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                                    │
│┌album records (value not formatted as JSON)──────────────────────────────────────────────────────┐ │
││key                       value                                                                  │ │
││album:1234567:890123     Pink Floyd|Dark Side Moon|1973|10|Progressive                           │ │
││album:1234567:890124     Led Zeppelin|IV|1971|8|Rock                                             │ │
││album:1234567:890125     Miles Davis|Kind of Blue|1959|5|Jazz                                    │ │
││album:1234567:890126     Beatles|Abbey Road|1969|17|Rock                                         │ │
││album:1234567:890127     Queen|Night at Opera|1975|12|Rock                                       │ │
│└─────────────────────────────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Navigation

- Up/Down arrows: Navigate records
- Left/Right arrows: Scroll horizontally
- PageUp/PageDown: Scroll pages
- Enter: Apply filter
- Backspace: Edit filter
- Double-click: View detailed record data
- d: Delete selected record (when database is unlocked)
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