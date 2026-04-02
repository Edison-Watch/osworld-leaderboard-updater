use calamine::{Reader, Xlsx, Data};
use chrono::Utc;
use std::io::Cursor;

const XLSX_URL: &str =
    "https://os-world.github.io/static/data/osworld_verified_results.xlsx";

struct Entry {
    model: String,
    institution: String,
    max_steps: String,
    success_rate: f64,
}

fn fetch_xlsx() -> Vec<u8> {
    let resp = reqwest::blocking::get(XLSX_URL).expect("failed to fetch xlsx");
    let status = resp.status();
    assert!(status.is_success(), "HTTP {status} fetching xlsx");
    resp.bytes().expect("failed to read response body").to_vec()
}

fn parse_entries(data: &[u8]) -> Vec<Entry> {
    let cursor = Cursor::new(data);
    let mut workbook: Xlsx<_> = Xlsx::new(cursor).expect("failed to parse xlsx");

    let sheet = workbook
        .worksheet_range("Eval Results")
        .expect("sheet 'Eval Results' not found");

    let rows: Vec<Vec<Data>> = sheet.rows().map(|r| r.to_vec()).collect();
    if rows.is_empty() {
        return vec![];
    }

    // Find column indices from header row
    let header: Vec<String> = rows[0].iter().map(|c| cell_to_string(c).trim().to_string()).collect();
    let col = |name: &str| header.iter().position(|h| h == name);

    let i_model = col("Model").expect("missing Model column");
    let i_institution = col("Institution").expect("missing Institution column");
    let i_approach = col("Approach type").expect("missing Approach type column");
    let i_a11y = col("Additional a11y tree used").expect("missing a11y column");
    let i_coding = col("Additional coding-based action").expect("missing coding column");
    let i_rollout = col("Multiple rollout").expect("missing rollout column");
    let i_max_steps = col("Max steps").expect("missing Max steps column");
    let i_success = col("Success rate").expect("missing Success rate column");

    let mut entries: Vec<Entry> = Vec::new();

    for row in &rows[1..] {
        let get = |i: usize| -> String {
            row.get(i).map(|c| cell_to_string(c).trim().to_string()).unwrap_or_default()
        };

        // Filter: Foundation E2E GUI
        if !get(i_approach).eq_ignore_ascii_case("General model")
            || !get(i_a11y).eq_ignore_ascii_case("no")
            || !get(i_coding).eq_ignore_ascii_case("no")
            || !get(i_rollout).eq_ignore_ascii_case("no")
        {
            continue;
        }

        let success_str = get(i_success);
        let success_rate = match parse_success_rate(&success_str) {
            Some(v) => v,
            None => continue,
        };

        entries.push(Entry {
            model: get(i_model),
            institution: get(i_institution),
            max_steps: get(i_max_steps),
            success_rate,
        });
    }

    entries.sort_by(|a, b| b.success_rate.total_cmp(&a.success_rate));
    entries.truncate(4);
    entries
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.clone(),
        Data::Float(f) => format!("{f}"),
        Data::Int(i) => format!("{i}"),
        Data::Bool(b) => format!("{b}"),
        Data::DateTime(dt) => format!("{dt}"),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e:?}"),
        Data::Empty => String::new(),
    }
}

fn parse_success_rate(s: &str) -> Option<f64> {
    let has_percent = s.contains('%');
    let cleaned = s.replace('%', "").trim().to_string();
    let v = cleaned.parse::<f64>().ok()?;
    if v.is_nan() || v.is_infinite() {
        return None;
    }
    if has_percent {
        // Value was already expressed as a percentage (e.g. "12.17%")
        Some(v)
    } else {
        // If calamine returned the raw Excel fraction (0..=1) instead of a
        // percentage integer (0..=100), scale it up.
        Some(if v > 0.0 && v <= 1.0 { v * 100.0 } else { v })
    }
}

fn render_svg(entries: &[Entry]) -> String {
    let col_widths: [f64; 5] = [60.0, 260.0, 200.0, 100.0, 140.0];
    let table_width: f64 = col_widths.iter().sum();
    let row_height = 36.0;
    let header_height = 40.0;
    let padding_top = 16.0;
    let padding_bottom = 30.0;
    let table_x = (800.0 - table_width) / 2.0;
    let table_y = padding_top;

    let body_height = entries.len() as f64 * row_height;
    let total_height = padding_top + header_height + body_height + padding_bottom;

    let mut svg = String::new();

    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="{total_height}" viewBox="0 0 800 {total_height}">
<style>
  text {{ font-family: Inter, Helvetica, Roboto, sans-serif; }}
</style>
<rect width="800" height="{total_height}" fill="#000000"/>
"##
    ));

    // Table border
    svg.push_str(&format!(
        r##"<rect x="{table_x}" y="{table_y}" width="{table_width}" height="{}" fill="none" stroke="#C3FFFD" stroke-width="1"/>
"##,
        header_height + body_height
    ));

    // Header row background
    svg.push_str(&format!(
        r##"<rect x="{table_x}" y="{table_y}" width="{table_width}" height="{header_height}" fill="#1C1C1C"/>
"##
    ));

    // Header separator line
    let header_bottom = table_y + header_height;
    svg.push_str(&format!(
        r##"<line x1="{table_x}" y1="{header_bottom}" x2="{}" y2="{header_bottom}" stroke="#C3FFFD" stroke-width="1"/>
"##,
        table_x + table_width
    ));

    // Column headers
    let headers = ["Rank", "Model", "Institution", "Max Steps", "Success Rate"];
    let mut cx = table_x;
    for (i, &label) in headers.iter().enumerate() {
        let text_x = cx + 12.0;
        let text_y = table_y + header_height / 2.0 + 5.0;
        svg.push_str(&format!(
            r##"<text x="{text_x}" y="{text_y}" fill="#C3FFFD" font-size="13" font-weight="600">{label}</text>
"##
        ));
        cx += col_widths[i];

        if i < headers.len() - 1 {
            svg.push_str(&format!(
                r##"<line x1="{cx}" y1="{table_y}" x2="{cx}" y2="{}" stroke="#C3FFFD" stroke-width="0.5" opacity="0.3"/>
"##,
                table_y + header_height + body_height
            ));
        }
    }

    // Body rows
    for (idx, entry) in entries.iter().enumerate() {
        let rank = idx + 1;
        let row_y = header_bottom + idx as f64 * row_height;

        if rank == 1 {
            svg.push_str(&format!(
                r##"<rect x="{table_x}" y="{row_y}" width="{table_width}" height="{row_height}" fill="#C3FFFD" opacity="0.08"/>
"##
            ));
        }

        if idx > 0 {
            svg.push_str(&format!(
                r##"<line x1="{table_x}" y1="{row_y}" x2="{}" y2="{row_y}" stroke="#C3FFFD" stroke-width="0.5" opacity="0.15"/>
"##,
                table_x + table_width
            ));
        }

        let text_y = row_y + row_height / 2.0 + 5.0;
        let mut cx = table_x;

        // Rank
        svg.push_str(&format!(
            r##"<text x="{}" y="{text_y}" fill="#F9F9F9" font-size="13" font-weight="500">{rank}</text>
"##,
            cx + 12.0
        ));
        cx += col_widths[0];

        // Model
        svg.push_str(&format!(
            r##"<text x="{}" y="{text_y}" fill="#F9F9F9" font-size="13">{}</text>
"##,
            cx + 12.0,
            escape_xml(&entry.model)
        ));
        cx += col_widths[1];

        // Institution
        svg.push_str(&format!(
            r##"<text x="{}" y="{text_y}" fill="#9BA4A6" font-size="12">{}</text>
"##,
            cx + 12.0,
            escape_xml(&entry.institution)
        ));
        cx += col_widths[2];

        // Max Steps
        svg.push_str(&format!(
            r##"<text x="{}" y="{text_y}" fill="#F9F9F9" font-size="13">{}</text>
"##,
            cx + 12.0,
            escape_xml(&entry.max_steps)
        ));
        cx += col_widths[3];

        // Success Rate
        svg.push_str(&format!(
            r##"<text x="{}" y="{text_y}" fill="#C3FFFD" font-size="13" font-weight="700">{:.1}%</text>
"##,
            cx + 12.0,
            entry.success_rate
        ));
    }

    // Updated timestamp
    let now = Utc::now().format("%Y-%m-%d");
    svg.push_str(&format!(
        r##"<text x="780" y="{}" fill="#5E6575" font-size="10" text-anchor="end">Updated: {now}</text>
"##,
        total_height - 8.0
    ));

    svg.push_str("</svg>\n");
    svg
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn main() {
    let mut output_path = String::from("assets/osworld-leaderboard.svg");

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--output" {
            i += 1;
            if i < args.len() {
                output_path = args[i].clone();
            }
        }
        i += 1;
    }

    eprintln!("Fetching leaderboard data...");
    let data = fetch_xlsx();

    eprintln!("Parsing entries...");
    let entries = parse_entries(&data);

    if entries.is_empty() {
        eprintln!("No matching entries found.");
        std::process::exit(1);
    }

    eprintln!("Top {} entries:", entries.len());
    for (i, e) in entries.iter().enumerate() {
        eprintln!("  {}. {} ({}) — {:.1}%", i + 1, e.model, e.institution, e.success_rate);
    }

    let svg = render_svg(&entries);

    if let Some(parent) = std::path::Path::new(&output_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    std::fs::write(&output_path, &svg).expect("failed to write SVG");
    eprintln!("Wrote {output_path}");
}
