"""Generate benchmark bar chart SVGs for README (dark and light variants)."""

BENCHMARKS = [
    {"label": "doxr", "ms": 110, "color": "#4ade80"},
    {"label": "mkdocs build --strict", "ms": 51000, "color": "#6b7280"},
]

PROJECT = "tinygrad"
FILES = "697 Python files"
SPEEDUP = "~460x faster"


def generate_svg(dark: bool) -> str:
    bg = "#0d1117" if dark else "#ffffff"
    text_color = "#e6edf3" if dark else "#1f2328"
    sub_color = "#8b949e" if dark else "#656d76"
    bar_bg = "#161b22" if dark else "#f6f8fa"
    border = "#30363d" if dark else "#d0d7de"

    width = 640
    height = 180
    margin_left = 200
    margin_right = 40
    bar_height = 32
    bar_gap = 24
    top = 60

    max_ms = max(b["ms"] for b in BENCHMARKS)
    bar_area = width - margin_left - margin_right

    bars_svg = ""
    for i, b in enumerate(BENCHMARKS):
        y = top + i * (bar_height + bar_gap)
        bar_width = max((b["ms"] / max_ms) * bar_area, 4)

        # Label
        bars_svg += f'  <text x="{margin_left - 12}" y="{y + bar_height / 2 + 5}" '
        bars_svg += f'font-family="system-ui, -apple-system, sans-serif" font-size="14" '
        bars_svg += f'fill="{text_color}" text-anchor="end">{b["label"]}</text>\n'

        # Bar background
        bars_svg += f'  <rect x="{margin_left}" y="{y}" width="{bar_area}" '
        bars_svg += f'height="{bar_height}" rx="4" fill="{bar_bg}"/>\n'

        # Bar
        bars_svg += f'  <rect x="{margin_left}" y="{y}" width="{bar_width:.1f}" '
        bars_svg += f'height="{bar_height}" rx="4" fill="{b["color"]}"/>\n'

        # Time label
        if b["ms"] >= 1000:
            time_str = f'{b["ms"] / 1000:.1f}s'
        else:
            time_str = f'{b["ms"]}ms'

        label_x = margin_left + bar_width + 8
        bars_svg += f'  <text x="{label_x}" y="{y + bar_height / 2 + 5}" '
        bars_svg += f'font-family="system-ui, -apple-system, sans-serif" font-size="13" '
        bars_svg += f'fill="{sub_color}">{time_str}</text>\n'

    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}">
  <rect width="{width}" height="{height}" rx="8" fill="{bg}" stroke="{border}" stroke-width="1"/>
  <text x="{width / 2}" y="28" font-family="system-ui, -apple-system, sans-serif" font-size="15" font-weight="600" fill="{text_color}" text-anchor="middle">Validating cross-references in {PROJECT} ({FILES})</text>
  <text x="{width / 2}" y="46" font-family="system-ui, -apple-system, sans-serif" font-size="12" fill="{sub_color}" text-anchor="middle">{SPEEDUP}</text>
{bars_svg}</svg>
"""


if __name__ == "__main__":
    import pathlib

    out = pathlib.Path(__file__).parent.parent / "assets"
    out.mkdir(exist_ok=True)

    (out / "benchmark-dark.svg").write_text(generate_svg(dark=True))
    (out / "benchmark-light.svg").write_text(generate_svg(dark=False))
    print("Generated assets/benchmark-dark.svg and assets/benchmark-light.svg")
