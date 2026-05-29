import sys
import re

# Simple console fallback visualization in case matplotlib/numpy are not installed on host
def draw_ascii_plot(bins):
    max_val = max(bins) if max(bins) > 0 else 1
    screen_width = 70
    print("\n" + "=" * 80)
    print(" LIVE FOLDED PULSE PROFILE (ASCII HISTOGRAM) ".center(80, "#"))
    print("=" * 80)
    # Downsample 1024 bins to 32 console rows
    step = len(bins) // 32
    for i in range(0, len(bins), step):
        chunk = bins[i:i+step]
        val = sum(chunk) // len(chunk)
        bar_length = int((val / max_val) * screen_width)
        bin_label = f"Bin {i:04d}: "
        print(bin_label + "*" * bar_length)
    print("=" * 80 + "\n")

def main():
    print("PulsarSync-Core Visualizer: Listening for RTT stream from stdin...")

    # Pre-allocate 1024 profile bins
    bins = [0] * 1024

    # Regex to capture RTT log format: "PROFILE_BIN index value"
    pattern = re.compile(r"PROFILE_BIN\s+(\d+)\s+(\d+)")

    try:
        for line in sys.stdin:
            # Print standard output logs
            sys.stdout.write(line)

            # Match profile dump lines
            match = pattern.search(line)
            if match:
                index = int(match.group(1))
                val = int(match.group(2))
                if index < 1024:
                    bins[index] = val

                # Once we reach index 1023 (end of the profile dump), refresh our visualization
                if index == 1023:
                    draw_ascii_plot(bins)

    except KeyboardInterrupt:
        print("\nExiting visualizer.")

if __name__ == "__main__":
    main();
