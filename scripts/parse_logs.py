import re
import csv
import os

# --- Configuration ---
# V V V V V V V V V V V V V V V V V V V V V V V V
# MAKE SURE THIS PATH IS CORRECT!
LOG_FILE_PATH = 'logs.txt' # <--- CHANGE THIS if your log file has a different name/location
# ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^ ^
OUTPUT_CSV_PATH = 'grow_commands.csv'
# -------------------

# Updated Regex to handle potential ANSI color codes and variable spacing
# It captures the timestamp, then non-greedily matches *anything* until
# it finds the key phrase "Command invoked: /grow by ", then captures user/ID.
log_pattern = re.compile(
    r"\[(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}).*?" # Timestamp (Group 1), followed by non-greedy any chars
    # Match the literal string identifying the command log entry
    r"Command invoked: /grow by "
    r"(.*?) "                                    # Username (non-greedy) (Group 2)
    r"\(ID: (\d+)\)"                             # User ID (Group 3)
)

parsed_data = []

print(f"Reading log file: {LOG_FILE_PATH}")

if not os.path.exists(LOG_FILE_PATH):
    print(f"Error: Log file not found at '{LOG_FILE_PATH}'")
    print("Please ensure the file exists and the LOG_FILE_PATH variable is set correctly.")
    exit()

try:
    # Use 'utf-8' encoding, common for logs, but change if needed
    with open(LOG_FILE_PATH, 'r', encoding='utf-8', errors='ignore') as infile:
        for i, line in enumerate(infile):
            match = log_pattern.search(line)
            if match:
                timestamp_str = match.group(1)
                username = match.group(2)
                user_id = match.group(3)
                parsed_data.append({
                    'Timestamp': timestamp_str,
                    'UserID': user_id,
                    'Username': username
                })
            # Optional: Add a check for debugging if lines aren't matching
            # else:
            #    if "Command invoked: /grow" in line:
            #        print(f"DEBUG: Line {i+1} contains '/grow' but didn't match regex:")
            #        print(f"  >> {line.strip()}")

except Exception as e:
    print(f"Error reading log file: {e}")
    exit()

print(f"Found {len(parsed_data)} '/grow' command invocations.")

if not parsed_data:
    print("No relevant log entries found. Check:")
    print("  1. The LOG_FILE_PATH is correct.")
    print("  2. The log file actually contains lines matching the pattern 'Command invoked: /grow by ...'")
    print("  3. The regex pattern correctly handles your log format (including special characters like color codes).")
    exit()

# Write to CSV
try:
    with open(OUTPUT_CSV_PATH, 'w', newline='', encoding='utf-8') as outfile:
        fieldnames = ['Timestamp', 'UserID', 'Username']
        writer = csv.DictWriter(outfile, fieldnames=fieldnames)

        writer.writeheader()
        writer.writerows(parsed_data)
    print(f"Successfully created CSV file: {OUTPUT_CSV_PATH}")
except Exception as e:
    print(f"Error writing CSV file: {e}")