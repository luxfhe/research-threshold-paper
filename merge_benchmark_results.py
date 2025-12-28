import os
import re
import glob
import pandas as pd

# Function to extract parameters from the filename
def extract_params_from_filename(filename):
    pattern = r"delay_(\S+)_bw_(\S+)_n_(\d+)_k_(\d+)_m_(\d+)_b_(\d+)_party(\d+)\.txt"
    match = re.match(pattern, filename)
    if match:
        return {
            "delay": match.group(1),
            "bw": match.group(2),
            "n": int(match.group(3)),
            "k": int(match.group(4)),
            "m": int(match.group(5)),
            "b": int(match.group(6)),
            "party": int(match.group(7)),
        }
    return None

# Function to parse ctxt_per_job, calculate average milliseconds, and count rows
def parse_file(file):
    with open(file, "r") as f:
        lines = f.readlines()
        ctxt_per_job = int(lines[0].split("ctxt_per_job: ")[1].split(",")[0])  # Extract ctxt_per_job
        microseconds = [int(line.split("microseconds: ")[1]) for line in lines]
        num_rows = len(microseconds)  # Number of "Completed all iterations..." rows
        avg_milliseconds = sum(microseconds) / (num_rows * 1000) if microseconds else 0  # Convert to ms
        return avg_milliseconds, ctxt_per_job, num_rows

# Main function to process all files
def process_files(directory):
    files = glob.glob(os.path.join(directory, "*.txt"))
    results = []

    for file in files:
        params = extract_params_from_filename(os.path.basename(file))
        if params:
            avg_milliseconds, ctxt_per_job, num_rows = parse_file(file)
            params["average_milliseconds"] = avg_milliseconds
            params["ctxt_per_job"] = ctxt_per_job
            params["num_jobs"] = num_rows
            results.append(params)

    # Convert to DataFrame for easier analysis
    df = pd.DataFrame(results)

    # Aggregate by unique sets of delay, bw, n, k, m, b (ignoring party)
    aggregated = (
        df.groupby(["delay", "bw", "n", "k", "m", "b", "ctxt_per_job"])
        .agg(
            overall_average_milliseconds=("average_milliseconds", "mean"),
            num_parties=("party", "count"),
            total_jobs=("num_jobs", "sum"),
        )
        .reset_index()
    )

    # Calculate avg_jobs_per_party and total_contexts_per_party
    aggregated["avg_jobs_per_party"] = (aggregated["total_jobs"] / aggregated["num_parties"]).round().astype(int)
    aggregated["total_contexts_per_party"] = aggregated["ctxt_per_job"] * aggregated["avg_jobs_per_party"]

    # Calculate tasks per second
    aggregated["tasks_per_second"] = (
        aggregated["total_contexts_per_party"] / (aggregated["overall_average_milliseconds"] / 1000)
    ).round(2)

    # Drop the original total_jobs for clarity
    aggregated.drop(columns=["total_jobs"], inplace=True)

    return df, aggregated

# Directory containing the text files
directory = "benchmark_results"

# Process the files
file_results, aggregated_results = process_files(directory)

# Save results to CSV for further analysis
file_results.to_csv("individual_file_averages.csv", index=False)
aggregated_results.to_csv("aggregated_averages.csv", index=False)

# Display or print results
print("Individual File Averages:")
print(file_results)

print("\nAggregated Averages:")
print(aggregated_results)
