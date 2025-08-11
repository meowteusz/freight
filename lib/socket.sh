#!/bin/bash
# Socket communication helpers for freight tools

# Socket configuration
SOCKET_PATH="/tmp/freight-daemon.sock"
SOCKET_RETRY_INTERVAL=10
MAX_SOCKET_RETRIES=3

# Socket connection state
SOCKET_CONNECTED=false
SOCKET_MANUAL_MODE=false

# Initialize socket communication
socket_init() {
    local tool_name="$1"
    local directory="$2"
    
    export TOOL_NAME="$tool_name"
    export CURRENT_DIRECTORY="$directory"
    
    # Test initial connection
    if socket_test_connection; then
        SOCKET_CONNECTED=true
        log_info "Connected to freight daemon"
        socket_hello "$tool_name" "$directory"
    else
        SOCKET_CONNECTED=false
        SOCKET_MANUAL_MODE=true
        log_warn "Cannot connect to freight daemon - running in manual mode"
    fi
}

# Test if socket connection is available
socket_test_connection() {
    if [[ -S "$SOCKET_PATH" ]]; then
        # Try to connect and send a test message
        echo "TEST" | nc -U "$SOCKET_PATH" -w 1 >/dev/null 2>&1
        return $?
    else
        return 1
    fi
}

# Send a message to the socket with retry logic
socket_send() {
    local message="$1"
    local retry_count=0
    
    # If in manual mode, don't try to send
    if [[ "$SOCKET_MANUAL_MODE" == "true" ]]; then
        log_debug "Socket message (manual mode): $message"
        return 0
    fi
    
    while (( retry_count < MAX_SOCKET_RETRIES )); do
        if echo "$message" | nc -U "$SOCKET_PATH" -w 1 >/dev/null 2>&1; then
            SOCKET_CONNECTED=true
            log_debug "Socket message sent: $message"
            return 0
        else
            SOCKET_CONNECTED=false
            ((retry_count++))
            
            if (( retry_count < MAX_SOCKET_RETRIES )); then
                log_warn "Socket send failed, retrying in ${SOCKET_RETRY_INTERVAL}s (attempt $retry_count/$MAX_SOCKET_RETRIES)"
                sleep "$SOCKET_RETRY_INTERVAL"
            else
                log_warn "Socket send failed after $MAX_SOCKET_RETRIES attempts, continuing without daemon"
                SOCKET_MANUAL_MODE=true
                return 1
            fi
        fi
    done
}

# Send hello message
socket_hello() {
    local tool="$1"
    local directory="$2"
    local hostname
    hostname="$(hostname)"
    local pid="$$"
    
    local message="HELLO freight/0.1.0 host=$hostname pid=$pid"
    socket_send "$message"
}

# Send start message
socket_start() {
    local tool="$1"
    local directory="$2"
    
    local message="START tool=$tool dir=$directory"
    socket_send "$message"
}

# Send progress message
socket_progress() {
    local tool="$1"
    local directory="$2"
    local progress_message="$3"
    local bytes_processed="${4:-}"
    
    local message="PROGRESS tool=$tool dir=$directory msg=$progress_message"
    
    if [[ -n "$bytes_processed" ]]; then
        message="$message bytes=$bytes_processed"
    fi
    
    socket_send "$message"
}

# Send stop message
socket_stop() {
    local tool="$1"
    local directory="$2"
    local status="$3"
    local bytes_processed="${4:-}"
    local final_message="${5:-}"
    
    local message="STOP tool=$tool dir=$directory status=$status"
    
    if [[ -n "$bytes_processed" ]]; then
        message="$message bytes=$bytes_processed"
    fi
    
    if [[ -n "$final_message" ]]; then
        message="$message msg=$final_message"
    fi
    
    socket_send "$message"
}

# Send custom message
socket_custom() {
    local message="$1"
    socket_send "$message"
}

# Check if socket is connected
socket_is_connected() {
    [[ "$SOCKET_CONNECTED" == "true" ]]
}

# Check if running in manual mode
socket_is_manual_mode() {
    [[ "$SOCKET_MANUAL_MODE" == "true" ]]
}

# Periodic socket health check
socket_health_check() {
    if [[ "$SOCKET_MANUAL_MODE" == "true" ]]; then
        # Try to reconnect if we were in manual mode
        if socket_test_connection; then
            log_info "Daemon connection restored"
            SOCKET_MANUAL_MODE=false
            SOCKET_CONNECTED=true
            # Re-send hello message
            socket_hello "$TOOL_NAME" "$CURRENT_DIRECTORY"
        fi
    elif [[ "$SOCKET_CONNECTED" == "true" ]]; then
        # Test existing connection
        if ! socket_test_connection; then
            log_warn "Lost connection to daemon"
            SOCKET_CONNECTED=false
        fi
    fi
}

# Background socket health monitor
socket_start_health_monitor() {
    local check_interval="${1:-30}"
    
    (
        while true; do
            sleep "$check_interval"
            socket_health_check
        done
    ) &
    
    local monitor_pid=$!
    log_debug "Started socket health monitor (PID: $monitor_pid)"
    
    # Store PID for cleanup
    echo "$monitor_pid" > "/tmp/freight-socket-monitor-$$"
}

# Stop socket health monitor
socket_stop_health_monitor() {
    local monitor_pid_file="/tmp/freight-socket-monitor-$$"
    
    if [[ -f "$monitor_pid_file" ]]; then
        local monitor_pid
        monitor_pid="$(cat "$monitor_pid_file")"
        
        if is_process_running "$monitor_pid"; then
            kill "$monitor_pid" 2>/dev/null
            log_debug "Stopped socket health monitor (PID: $monitor_pid)"
        fi
        
        rm -f "$monitor_pid_file"
    fi
}

# Cleanup socket resources
socket_cleanup() {
    socket_stop_health_monitor
    
    # Send final disconnect if connected
    if socket_is_connected; then
        socket_stop "$TOOL_NAME" "$CURRENT_DIRECTORY" "interrupted" "" "Process terminated"
    fi
}

# Set up socket cleanup on exit
socket_setup_cleanup() {
    trap 'socket_cleanup; cleanup_and_exit 130' INT  # Ctrl+C
    trap 'socket_cleanup; cleanup_and_exit 143' TERM # Termination
    trap 'socket_cleanup' EXIT # Normal exit
}