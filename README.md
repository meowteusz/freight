# Freight NFS Migration Suite

A modular NFS migration suite designed for efficient, resumable transfer of fragmented academic datasets between high-bandwidth NFS shares.

## Architecture

Freight follows Unix philosophy with independent, chainable components:

- **Rust Orchestrator** (`freight`) - Daemon-based process management with TUI dashboard
- **Bash Tools** - Core migration utilities that can run standalone or orchestrated
  - `freight-scan` - Directory snapshot scanner
  - `freight-clean` - Cache and temporary file cleaner  
  - `freight-migrate` - Migration engine using rsync
  - `freight-verify` - Integrity verifier
  - `freight-sync` - Incremental synchronizer

## Quick Start

### Build the Orchestrator

```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the freight orchestrator
cargo build --release

# The binary will be at target/release/freight
```

### Install Tools

```bash
# Make bash tools executable
chmod +x bin/*

# Optionally install to system PATH
sudo cp bin/* /usr/local/bin/
sudo cp target/release/freight /usr/local/bin/
```

### Basic Usage

```bash
# Start migration with dashboard
freight migrate /nfs1/students /nfs2/students

# Or run daemon in background and connect TUI separately
freight daemon &
freight connect

# Run individual tools standalone
freight-scan /path/to/directory
freight-clean --dry-run /path/to/directory
```

## Commands

### Orchestrator Commands

```bash
freight dashboard                    # Start daemon + TUI dashboard
freight migrate <source> <dest>      # Start migration with dashboard
freight daemon [--foreground]       # Start daemon only
freight connect                     # Connect TUI to existing daemon
```

### Tool Commands

```bash
freight-scan [options] [directory]           # Scan and catalog directory
freight-clean [options] [directory]          # Clean cache/temp directories
freight-migrate [options] <source> <dest>    # Migrate directory (requires daemon)
freight-verify [options] <source> <dest>     # Verify migration integrity
freight-sync [options] <source> <dest>       # Incremental sync (requires daemon)
```

## Configuration

Configuration is stored in `.freight/config.json` at the migration root:

```json
{
  "source_path": "/nfs1/students",
  "dest_path": "/nfs2/students",
  "thresholds": {
    "large_directory_size": "3GB",
    "parallel_workers": 5
  },
  "rsync_flags": "-avxHAX --numeric-ids --compress",
  "retry_attempts": 3,
  "socket_retry_interval": 10
}
```

## Directory Structure

```
/nfs1/students/           # Migration root
├── .freight/
│   ├── .freight-root     # Marker file
│   └── config.json       # Configuration
├── alice/               # Student directory
│   └── .freight/        # Per-directory metadata
│       ├── scan.json    # Scan results
│       ├── migrate.json # Migration log
│       ├── verify.json  # Verification results
│       └── sync.json    # Sync operations
└── bob/                 # Another student directory
    └── .freight/
```

## Development

### Build Commands

```bash
# Build orchestrator
cargo build --release

# Run tests
cargo test

# Install bash tools
chmod +x bin/*

# Check code formatting
cargo fmt --check

# Run linter
cargo clippy
```

### Architecture Principles

- **Unix Doctrine**: Each tool does one thing well
- **Modularity**: Independent, chainable components  
- **Resumability**: All operations support graceful recovery
- **Observability**: Comprehensive logging and progress tracking

### Communication

- Unix domain socket (`/tmp/freight-daemon.sock`) for inter-process communication
- Filesystem `.freight/*.json` for persistent state and logs
- Socket failures: retry every 10 seconds, graceful degradation for optional tools
- Hard fail for migration/sync tools if orchestrator unavailable

## Examples

### Complete Migration Workflow

```bash
# 1. Start with scanning
freight-scan /nfs1/students

# 2. Clean unnecessary files
freight-clean --dry-run /nfs1/students  # Preview
freight-clean /nfs1/students             # Execute

# 3. Start orchestrated migration
freight migrate /nfs1/students /nfs2/students

# 4. Verify migration integrity
freight-verify /nfs1/students /nfs2/students

# 5. Incremental sync for live changes
freight-sync /nfs1/students /nfs2/students
```

### Standalone Tool Usage

```bash
# Scan specific directory
freight-scan --verbose /nfs1/students/alice

# Clean with custom patterns
freight-clean --pattern "*.tmp" --pattern ".cache" /data

# Migrate with custom rsync flags
freight-migrate --rsync-flags "-av --compress" /src /dst

# Verify with checksum sampling
freight-verify --mode checksum --sample-rate 25 /src /dst
```

## Troubleshooting

### Common Issues

1. **Socket connection failed**: Start the freight daemon first
2. **Permission denied**: Ensure read/write access to source and destination
3. **Migration stalled**: Check network connectivity and disk space
4. **Verification failed**: Review `.freight/verify.json` for detailed discrepancies

### Logs

- Tool logs: `.freight/<tool>.log` in each directory
- Structured JSON results: `.freight/<tool>.json`
- Daemon logs: Check systemd journal or console output