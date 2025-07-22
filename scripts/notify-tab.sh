#!/bin/bash

# Send notification to a Zellij tab
# Usage: notify-tab.sh <tab_number>
#    or: notify-tab.sh <tab_name>

if [ $# -eq 0 ]; then
    echo "Usage: $0 <tab_number> or $0 <tab_name>"
    echo "Examples:"
    echo "  $0 2        # Notify tab 2"
    echo "  $0 main     # Notify tab named 'main'"
    exit 1
fi

ARG="$1"

# Check if argument is a number
if [[ "$ARG" =~ ^[0-9]+$ ]]; then
    # It's a tab number
    zellij pipe --name "zj-status-sidebar:cli:notify" --args "tab=$ARG"
else
    # It's a tab name
    zellij pipe --name "zj-status-sidebar:cli:notify" --args "tab_name=$ARG"
fi