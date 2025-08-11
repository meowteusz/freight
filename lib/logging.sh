#!/bin/bash
# Structured logging utilities for freight tools

# Log levels
LOG_LEVEL_DEBUG=0
LOG_LEVEL_INFO=1
LOG_LEVEL_WARN=2
LOG_LEVEL_ERROR=3

# Current log level (default to INFO)
CURRENT_LOG_LEVEL=${FREIGHT_LOG_LEVEL:-$LOG_LEVEL_INFO}

# Log file path (if set)
LOG_FILE="${FREIGHT_LOG_FILE:-}"

# Get log level name
get_log_level_name() {
    case "$1" in
        $LOG_LEVEL_DEBUG) echo "DEBUG" ;;
        $LOG_LEVEL_INFO) echo "INFO" ;;
        $LOG_LEVEL_WARN) echo "WARN" ;;
        $LOG_LEVEL_ERROR) echo "ERROR" ;;
        *) echo "UNKNOWN" ;;
    esac
}

# Core logging function
log_message() {
    local level="$1"
    local message="$2"
    local tool_name="${TOOL_NAME:-freight}"
    local timestamp
    timestamp="$(get_timestamp)"
    
    # Check if we should log this level
    if (( level < CURRENT_LOG_LEVEL )); then
        return 0
    fi
    
    local level_name
    level_name="$(get_log_level_name "$level")"
    
    # Format: timestamp [LEVEL] tool: message
    local log_entry="$timestamp [$level_name] $tool_name: $message"
    
    # Color output for terminal
    local color=""
    case "$level" in
        $LOG_LEVEL_DEBUG) color="$BLUE" ;;
        $LOG_LEVEL_INFO) color="$GREEN" ;;
        $LOG_LEVEL_WARN) color="$YELLOW" ;;
        $LOG_LEVEL_ERROR) color="$RED" ;;
    esac
    
    # Output to stderr for levels WARN and above, stdout for others
    if (( level >= LOG_LEVEL_WARN )); then
        echo -e "${color}${log_entry}${NC}" >&2
    else
        echo -e "${color}${log_entry}${NC}"
    fi
    
    # Also write to log file if specified
    if [[ -n "$LOG_FILE" ]]; then
        echo "$log_entry" >> "$LOG_FILE"
    fi
}

# Convenience logging functions
log_debug() {
    log_message $LOG_LEVEL_DEBUG "$1"
}

log_info() {
    log_message $LOG_LEVEL_INFO "$1"
}

log_warn() {
    log_message $LOG_LEVEL_WARN "$1"
}

log_error() {
    log_message $LOG_LEVEL_ERROR "$1"
}

# Log with JSON structure for machine parsing
log_json() {
    local level="$1"
    local message="$2"
    local extra_fields="$3"
    local tool_name="${TOOL_NAME:-freight}"
    local timestamp
    timestamp="$(get_timestamp)"
    
    # Build JSON object
    local json_log="{\"timestamp\":\"$timestamp\",\"level\":\"$(get_log_level_name "$level")\",\"tool\":\"$tool_name\",\"message\":\"$message\""
    
    if [[ -n "$extra_fields" ]]; then
        json_log="$json_log,$extra_fields"
    fi
    
    json_log="$json_log}"
    
    # Output JSON log
    echo "$json_log"
    
    # Also write to log file if specified
    if [[ -n "$LOG_FILE" ]]; then
        echo "$json_log" >> "$LOG_FILE"
    fi
}

# Log operation start
log_operation_start() {
    local operation="$1"
    local target="$2"
    local operation_id="${3:-$(generate_uuid)}"
    
    log_json $LOG_LEVEL_INFO "Operation started" \
        "\"operation\":\"$operation\",\"target\":\"$target\",\"operation_id\":\"$operation_id\""
    
    echo "$operation_id"
}

# Log operation end
log_operation_end() {
    local operation="$1"
    local target="$2"
    local operation_id="$3"
    local status="$4"
    local duration="$5"
    local bytes_processed="${6:-0}"
    
    log_json $LOG_LEVEL_INFO "Operation completed" \
        "\"operation\":\"$operation\",\"target\":\"$target\",\"operation_id\":\"$operation_id\",\"status\":\"$status\",\"duration\":$duration,\"bytes_processed\":$bytes_processed"
}

# Log progress update
log_progress() {
    local operation="$1"
    local target="$2"
    local operation_id="$3"
    local progress_message="$4"
    local bytes_processed="${5:-0}"
    local percentage="${6:-}"
    
    local extra_fields="\"operation\":\"$operation\",\"target\":\"$target\",\"operation_id\":\"$operation_id\",\"bytes_processed\":$bytes_processed"
    
    if [[ -n "$percentage" ]]; then
        extra_fields="$extra_fields,\"percentage\":$percentage"
    fi
    
    log_json $LOG_LEVEL_INFO "$progress_message" "$extra_fields"
}

# Initialize logging for a tool
init_logging() {
    local tool_name="$1"
    local log_dir="$2"
    
    export TOOL_NAME="$tool_name"
    
    # Set up log file if log directory is provided
    if [[ -n "$log_dir" ]]; then
        ensure_freight_dir "$log_dir"
        export LOG_FILE="$log_dir/${tool_name}.log"
        log_info "Logging initialized for $tool_name, log file: $LOG_FILE"
    else
        log_info "Logging initialized for $tool_name (console only)"
    fi
}

# Log system information
log_system_info() {
    log_info "System: $(uname -a)"
    log_info "User: $(whoami)"
    log_info "Working directory: $PWD"
    log_info "Process ID: $$"
    log_info "Parent process ID: $PPID"
}

# Log configuration
log_config() {
    local config_file="$1"
    
    if [[ -f "$config_file" ]]; then
        log_info "Configuration loaded from: $config_file"
        log_debug "Configuration contents: $(cat "$config_file")"
    else
        log_warn "Configuration file not found: $config_file"
    fi
}

# Log error with stack trace (if available)
log_error_with_trace() {
    local message="$1"
    local line_number="${2:-$LINENO}"
    local function_name="${3:-${FUNCNAME[1]}}"
    local script_name="${4:-$0}"
    
    log_error "$message (at $script_name:$function_name:$line_number)"
    
    # If we have a stack trace, log it
    if [[ -n "${BASH_SOURCE[*]}" ]]; then
        log_debug "Stack trace:"
        local i=1
        while [[ -n "${BASH_SOURCE[$i]}" ]]; do
            log_debug "  $i: ${BASH_SOURCE[$i]}:${FUNCNAME[$i]}:${BASH_LINENO[$((i-1))]}"
            ((i++))
        done
    fi
}