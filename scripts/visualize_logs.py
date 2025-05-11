import pandas as pd
import plotly.express as px
import plotly.graph_objects as go # For adding shapes/lines
import os

# --- Configuration ---
CSV_FILE_PATH = 'grow_commands.csv' # Input CSV from the previous script
OUTPUT_HTML_PATH = 'grow_command_intervals.html' # Output HTML plot
COOLDOWN_MINUTES = 60
# -------------------

print(f"Reading CSV file: {CSV_FILE_PATH}")

if not os.path.exists(CSV_FILE_PATH):
    print(f"Error: CSV file not found at '{CSV_FILE_PATH}'. Did you run the parsing script first?")
    exit()

try:
    df = pd.read_csv(CSV_FILE_PATH)
except Exception as e:
    print(f"Error reading CSV file: {e}")
    exit()

if df.empty:
    print("CSV file is empty. No data to visualize.")
    exit()

# Convert Timestamp column to datetime objects
try:
    df['Timestamp'] = pd.to_datetime(df['Timestamp'])
except Exception as e:
    print(f"Error converting 'Timestamp' column to datetime: {e}")
    print("Please ensure the timestamp format in the CSV is correct (YYYY-MM-DD HH:MM:SS).")
    exit()

# Sort data first by UserID, then by Timestamp is CRITICAL for diff()
df = df.sort_values(by=['UserID', 'Timestamp'])

# Calculate the time difference between consecutive commands *for each user*
# .diff() calculates the difference with the *previous* row within each group
df['TimeDelta'] = df.groupby('UserID')['Timestamp'].diff()

# Convert TimeDelta to total seconds for easier plotting
# Fill NaN values (first command per user) with a value that won't interfere,
# or just drop them before plotting if preferred. We'll keep them for now,
# but filter them out before plotting the histogram.
df['TimeDeltaSeconds'] = df['TimeDelta'].dt.total_seconds()

# Filter out the NaN values (first command for each user has no previous command)
plot_data = df['TimeDeltaSeconds'].dropna()

if plot_data.empty:
    print("No time differences could be calculated (e.g., each user only used the command once).")
    exit()

print(f"Calculated {len(plot_data)} time intervals between commands.")

# Create the histogram
print("Generating histogram...")
cooldown_seconds = COOLDOWN_MINUTES * 60
max_seconds = plot_data.max()
# Choose reasonable bins, maybe every 5 minutes up to a bit past the max observed?
# Or let Plotly decide automatically, which is often good enough.
# Let's set an explicit range up to maybe 1 hour past cooldown if max is small,
# or a bit past the observed max otherwise.
plot_range_max = max(cooldown_seconds * 2, max_seconds * 1.1)

fig = px.histogram(
    plot_data,
    nbins=10000, # Adjust number of bins as needed for granularity
    range_x=[0, plot_range_max], # Set a sensible x-axis range
    title='Distribution of Time Between Consecutive /grow Commands (Per User)',
    labels={'value': 'Time Difference (seconds)'}, # Renames the x-axis label derived from the data column name
    opacity=0.8,
)

# Add a vertical line at the cooldown period (30 minutes = 1800 seconds)
fig.add_vline(
    x=cooldown_seconds,
    line_width=2,
    line_dash="dash",
    line_color="red",
    annotation_text=f"{COOLDOWN_MINUTES} min Cooldown",
    annotation_position="top right"
)

fig.update_layout(
    xaxis_title="Time Between Consecutive Commands (seconds)",
    yaxis_title="Number of Occurrences",
    bargap=0.1 # Gap between bars
)

# Save to HTML file
try:
    fig.write_html(OUTPUT_HTML_PATH)
    print(f"Successfully created interactive HTML plot: {OUTPUT_HTML_PATH}")
    print("Open this file in your web browser.")
except Exception as e:
    print(f"Error writing HTML file: {e}")