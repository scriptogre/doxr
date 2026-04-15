"""Generate benchmark bar chart SVGs matching ruff's style."""

BENCHMARKS = [
    {"label": "doxr", "seconds": 0.110, "bold": True},
    {"label": "mkdocs build --strict", "seconds": 51.0, "bold": False},
]

CAPTION = None  # Caption is in the README, not the SVG


def format_time(s: float) -> str:
    if s == 0:
        return "0s"
    return f"{s:.2f}s" if s < 1 else f"{s:.1f}s"


def generate_svg(dark: bool) -> str:
    text_color = "#C9D1D9" if dark else "#24292f"
    grid_color = "rgba(127,127,127,0.25)" if dark else "rgba(127,127,127,0.2)"
    bar_color = "#6340AC" if dark else "#6340AC"
    font = '-apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif'

    # Layout
    width = 600
    left_margin = 170
    right_margin = 60
    top_margin = 25
    chart_width = width - left_margin - right_margin
    bar_height = 16
    bar_gap = 28
    chart_height = len(BENCHMARKS) * (bar_height + bar_gap) - bar_gap + 10
    axis_height = 25
    caption_height = 20 if CAPTION else 0
    total_height = top_margin + chart_height + axis_height + caption_height

    max_seconds = max(b["seconds"] for b in BENCHMARKS)
    # Round up to nice number for axis
    if max_seconds > 30:
        axis_max = 60
        tick_step = 20
    elif max_seconds > 10:
        axis_max = max_seconds * 1.15
        tick_step = 10
    else:
        axis_max = max_seconds * 1.15
        tick_step = 1

    ticks = []
    t = 0
    while t <= axis_max:
        ticks.append(t)
        t += tick_step

    def x_pos(seconds: float) -> float:
        return (seconds / axis_max) * chart_width

    svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {total_height}">\n'
    svg += f'  <g transform="translate({left_margin},{top_margin})">\n'

    # Grid lines
    for t in ticks:
        x = x_pos(t)
        svg += f'    <line x1="{x:.1f}" y1="0" x2="{x:.1f}" y2="{chart_height}" '
        svg += f'stroke="{grid_color}" stroke-width="1"/>\n'

    # Bars
    for i, b in enumerate(BENCHMARKS):
        y = i * (bar_height + bar_gap) + 5
        bw = max(x_pos(b["seconds"]), 2)
        weight = ' font-weight="bold"' if b["bold"] else ""

        # Label (left)
        svg += f'    <text x="-12" y="{y + bar_height / 2 + 4}" text-anchor="end" '
        svg += f'font-family="{font}" font-size="13px" fill="{text_color}"{weight}>'
        svg += f'{b["label"]}</text>\n'

        # Bar
        svg += f'    <rect x="0" y="{y}" width="{bw:.1f}" height="{bar_height}" '
        svg += f'rx="2" fill="{bar_color}"/>\n'

        # Value label (right of bar)
        svg += f'    <text x="{bw + 6:.1f}" y="{y + bar_height / 2 + 4}" text-anchor="start" '
        svg += f'font-family="{font}" font-size="12px" fill="{text_color}"{weight}>'
        svg += f'{format_time(b["seconds"])}</text>\n'

    # X-axis labels
    for t in ticks:
        x = x_pos(t)
        svg += f'    <text x="{x:.1f}" y="{chart_height + 18}" text-anchor="middle" '
        svg += f'font-family="{font}" font-size="11px" fill="{text_color}">'
        svg += f'{format_time(t)}</text>\n'

    svg += "  </g>\n"

    if CAPTION:
        svg += f'  <text x="{width / 2}" y="{total_height - 2}" text-anchor="middle" '
        svg += f'font-family="{font}" font-size="11px" fill="{text_color}" opacity="0.6">'
        svg += f'{CAPTION}</text>\n'

    svg += "</svg>\n"
    return svg


if __name__ == "__main__":
    import pathlib

    out = pathlib.Path(__file__).parent.parent / "assets"
    out.mkdir(exist_ok=True)

    (out / "benchmark-dark.svg").write_text(generate_svg(dark=True))
    (out / "benchmark-light.svg").write_text(generate_svg(dark=False))
    print("Generated assets/benchmark-dark.svg and assets/benchmark-light.svg")
