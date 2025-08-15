#!/bin/bash

# Script to analyze hash values from hashes.log
# Finds all occurrences where hash value is less than or equal to difficulty
# Prints hash value, difficulty, and job_id

LOG_FILE="hashes.log"

# Check if log file exists
if [ ! -f "$LOG_FILE" ]; then
    echo "Error: $LOG_FILE not found. Run the miner with --debug-all first to generate hash data."
    exit 1
fi

# Check if log file is empty
if [ ! -s "$LOG_FILE" ]; then
    echo "Error: $LOG_FILE is empty. Run the miner with --debug-all to generate hash data."
    exit 1
fi

echo "Searching for valid shares (hash_value <= difficulty)..."
echo "Format: hash_value <= difficulty (job_id)"
echo "----------------------------------------"

# Use awk to process the CSV file
# Fields: $1=nonce, $2=hash_value, $3=difficulty, $4=job_id
awk -F',' '$2 <= $3 { printf "%d <= %d (%s)\n", $2, $3, $4 }' "$LOG_FILE"

# Print summary
count=$(awk -F',' '$2 <= $3 { count++ } END { print count+0 }' "$LOG_FILE")
echo "----------------------------------------"
echo "Found $count valid shares"
