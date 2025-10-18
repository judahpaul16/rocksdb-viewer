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

```
┌Filter by Key──────────────────────────────────────────────────────────────────────────────────────────────┐
│artist:                                                                                                    │                              │                                                                                                           │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Records────────────────────────────────────────────────────────────────────────────────────────────────────┐
│┌Album Records───────────────────────────────────────────────────────────────────────────────────────────┐ │
││key                              artist           album               year    tracks    genre           │ │
││album:1234567:890123            Pink Floyd       Dark Side Moon     1973    10        Progressive       │ │
││album:1234567:890124            Led Zeppelin     IV                 1971    8         Rock              │ │
││album:1234567:890125            Miles Davis      Kind of Blue       1959    5         Jazz              │ │
││album:1234567:890126            Beatles          Abbey Road         1969    17        Rock              │ │
││album:1234567:890127            Queen            Night at Opera     1975    12        Rock              │ │
│└────────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                                           │
│┌Track Records────────────────────────────────────────────────────────────────────────────────────────────┐│
││key                        title              duration    plays    rating    last_played                 ││
││track:1234567:890123       Money              6:22       1250     4.8       2025-10-18 07:04:29          ││
││track:1234567:890124       Stairway Heaven    8:02       1890     4.9       2025-10-18 07:04:17          ││
││track:1234567:890125       So What            9:22       950      4.7       2025-10-18 07:04:10          ││
││track:1234567:890126       Come Together      4:19       1544     4.6       2025-10-18 07:04:03          ││
││track:1234567:890127       Rhapsody           5:55       2150     5.0       2025-10-18 07:03:41          ││
│└─────────────────────────────────────────────────────────────────────────────────────────────────────────┘│
└───────────────────────────────────────────────────────────────────────────────────────────────────────────┘
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