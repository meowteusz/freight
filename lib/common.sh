#!/bin/bash
# Common utilities for freight tools

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
FREIGHT_ROOT_MARKER=".freight-root"
FREIGHT_DIR=".freight"
SOCKET_PATH="/tmp/freight-daemon.sock"

# Find the freight root directory by looking for .freight-root marker
find_freight_root() {
    local current_dir="$PWD"
    
    while [[ "$current_dir" != "/" ]]; do
        if [[ -f "$current_dir/$FREIGHT_ROOT_MARKER" ]]; then
            echo "$current_dir"
            return 0
        fi
        current_dir="$(dirname "$current_dir")"
    done
    
    return 1
}

# Get the freight directory for the current context
get_freight_dir() {
    local target_dir="${1:-.}"
    echo "$target_dir/$FREIGHT_DIR"
}

# Ensure freight directory exists
ensure_freight_dir() {
    local freight_dir="$1"
    
    if [[ ! -d "$freight_dir" ]]; then
        mkdir -p "$freight_dir"
        log_info "Created freight directory: $freight_dir"
    fi
}

# Generate UUID for tracking operations
generate_uuid() {
    if command -v uuidgen >/dev/null 2>&1; then
        uuidgen | tr '[:upper:]' '[:lower:]'
    else
        # Fallback UUID generation
        cat /proc/sys/kernel/random/uuid 2>/dev/null || \
        python3 -c "import uuid; print(uuid.uuid4())" 2>/dev/null || \
        echo "$(date +%s)-$$-$(shuf -i 1000-9999 -n 1)"
    fi
}

# Get current timestamp in ISO format
get_timestamp() {
    date -u +"%Y-%m-%dT%H:%M:%SZ"
}

# Parse size string (e.g., "3GB") to bytes
parse_size_to_bytes() {
    local size_str="$1"
    local number="${size_str%[A-Za-z]*}"
    local unit="${size_str#$number}"
    
    case "${unit^^}" in
        "B"|"") echo "$number" ;;
        "KB") echo $((number * 1024)) ;;
        "MB") echo $((number * 1024 * 1024)) ;;
        "GB") echo $((number * 1024 * 1024 * 1024)) ;;
        "TB") echo $((number * 1024 * 1024 * 1024 * 1024)) ;;
        *) echo "$number" ;;
    esac
}

# Format bytes to human readable
format_bytes() {
    local bytes="$1"
    local units=("B" "KB" "MB" "GB" "TB")
    local unit_index=0
    local size="$bytes"
    
    while (( size >= 1024 && unit_index < 4 )); do
        size=$((size / 1024))
        ((unit_index++))
    done
    
    if (( unit_index == 0 )); then
        echo "${bytes} ${units[unit_index]}"
    else
        printf "%.1f %s\n" "$(echo "scale=1; $bytes / (1024^$unit_index)" | bc -l)" "${units[unit_index]}"
    fi
}

# Check if a process is running
is_process_running() {
    local pid="$1"
    kill -0 "$pid" 2>/dev/null
}

# Get directory size using du
get_directory_size() {
    local dir="$1"
    du -sb "$dir" 2>/dev/null | cut -f1
}

# Count files in directory
count_files() {
    local dir="$1"
    find "$dir" -type f 2>/dev/null | wc -l
}

# Validate rsync flags
validate_rsync_flags() {
    local flags="$1"
    
    # Check for required flags
    if [[ ! "$flags" =~ -a ]]; then
        log_warn "Missing -a (archive) flag in rsync command"
    fi
    
    if [[ ! "$flags" =~ --numeric-ids ]]; then
        log_warn "Missing --numeric-ids flag - this is critical for NFS migrations"
    fi
}

# Create a backup of a file
backup_file() {
    local file="$1"
    local backup_file="${file}.backup.$(date +%s)"
    
    if [[ -f "$file" ]]; then
        cp "$file" "$backup_file"
        log_info "Created backup: $backup_file"
    fi
}

# Cleanup function for signal handlers
cleanup_and_exit() {
    local exit_code="${1:-1}"
    log_info "Cleaning up and exiting with code $exit_code"
    exit "$exit_code"
}

# Set up signal handlers
setup_signal_handlers() {
    trap 'cleanup_and_exit 130' INT  # Ctrl+C
    trap 'cleanup_and_exit 143' TERM # Termination
}

# Check if running as root (for some operations)
check_root() {
    if [[ $EUID -eq 0 ]]; then
        return 0
    else
        return 1
    fi
}

# Validate directory exists and is readable
validate_directory() {
    local dir="$1"
    local operation="${2:-access}"
    
    if [[ ! -d "$dir" ]]; then
        log_error "Directory does not exist: $dir"
        return 1
    fi
    
    if [[ ! -r "$dir" ]]; then
        log_error "Directory is not readable: $dir"
        return 1
    fi
    
    if [[ "$operation" == "write" && ! -w "$dir" ]]; then
        log_error "Directory is not writable: $dir"
        return 1
    fi
    
    return 0
}