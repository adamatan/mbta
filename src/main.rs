use chrono::{DateTime, Duration, Local};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;

const BASE_URL: &str = "https://api-v3.mbta.com";

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct Resource<A, R> {
    attributes: A,
    relationships: R,
}

#[derive(Debug, Deserialize)]
struct ScheduleAttributes {
    arrival_time: Option<String>,
    departure_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScheduleRelationships {
    trip: DataWrapper,
}

#[derive(Debug, Deserialize)]
struct PredictionAttributes {
    arrival_time: Option<String>,
    departure_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PredictionRelationships {
    trip: DataWrapper,
}

#[derive(Debug, Deserialize)]
struct DataWrapper {
    data: IdWrapper,
}

#[derive(Debug, Deserialize)]
struct IdWrapper {
    id: String,
}

#[derive(Clone)]
struct StopConfig {
    route_id: &'static str,
    stop_id: &'static str,
    direction_id: i32,
    is_origin: bool,
}

#[derive(Debug, Clone)]
struct RowData {
    sched_dt: Option<DateTime<Local>>,
    pred_dt: Option<DateTime<Local>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let now = Local::now();

    // Define Stops - Route 60
    let stop_kenmore = StopConfig {
        route_id: "60",
        stop_id: "place-kencl",
        direction_id: 0,
        is_origin: true,
    };

    let stop_brookline_ave = StopConfig {
        route_id: "60",
        stop_id: "1519",
        direction_id: 0,
        is_origin: false,
    };

    let stop_pearl = StopConfig {
        route_id: "60",
        stop_id: "11366",
        direction_id: 0,
        is_origin: false,
    };

    let stop_high = StopConfig {
        route_id: "60",
        stop_id: "1553",
        direction_id: 1,
        is_origin: false,
    };

    // Define Stops - Green Line D
    let stop_copley = StopConfig {
        route_id: "Green-D",
        stop_id: "place-coecl",
        direction_id: 0,
        is_origin: true,
    };

    let stop_brookline = StopConfig {
        route_id: "Green-D",
        stop_id: "place-bvmnl",
        direction_id: 1,
        is_origin: true,
    };

    // 1. Fetch Data Concurrently
    let (res_kenmore, res_brookline_ave, res_pearl, res_high, res_copley, res_brookline) =
        tokio::join!(
            get_schedule_and_predictions(&client, &stop_kenmore, now),
            get_schedule_and_predictions(&client, &stop_brookline_ave, now),
            get_schedule_and_predictions(&client, &stop_pearl, now),
            get_schedule_and_predictions(&client, &stop_high, now),
            get_schedule_and_predictions(&client, &stop_copley, now),
            get_schedule_and_predictions(&client, &stop_brookline, now)
        );

    // Check for rate limiting first
    if res_kenmore.is_err() && res_kenmore.as_ref().unwrap_err().to_string() == "Rate limited"
        || res_brookline_ave.is_err()
            && res_brookline_ave.as_ref().unwrap_err().to_string() == "Rate limited"
        || res_pearl.is_err() && res_pearl.as_ref().unwrap_err().to_string() == "Rate limited"
        || res_high.is_err() && res_high.as_ref().unwrap_err().to_string() == "Rate limited"
        || res_copley.is_err() && res_copley.as_ref().unwrap_err().to_string() == "Rate limited"
        || res_brookline.is_err()
            && res_brookline.as_ref().unwrap_err().to_string() == "Rate limited"
    {
        eprintln!("⚠️  MBTA API rate limit exceeded. Please wait a moment and try again.");
        std::process::exit(1);
    }

    let rows_kenmore = res_kenmore.unwrap_or_else(|e| {
        eprintln!("⚠️  Error fetching Kenmore data: {}", e);
        vec![]
    });
    let rows_brookline_ave = res_brookline_ave.unwrap_or_else(|e| {
        eprintln!("⚠️  Error fetching Brookline Ave data: {}", e);
        vec![]
    });
    let rows_pearl = res_pearl.unwrap_or_else(|e| {
        eprintln!("⚠️  Error fetching Pearl St data: {}", e);
        vec![]
    });
    let rows_high = res_high.unwrap_or_else(|e| {
        eprintln!("⚠️  Error fetching High St data: {}", e);
        vec![]
    });
    let rows_copley = res_copley.unwrap_or_else(|e| {
        eprintln!("⚠️  Error fetching Copley data: {}", e);
        vec![]
    });
    let rows_brookline = res_brookline.unwrap_or_else(|e| {
        eprintln!("⚠️  Error fetching Brookline Village data: {}", e);
        vec![]
    });

    // Filter rows > 5 mins ago
    let filter_rows = |rows: Vec<RowData>| -> Vec<RowData> {
        rows.into_iter()
            .filter(|r| {
                let s_diff = r
                    .sched_dt
                    .map(|t| t.signed_duration_since(now).num_minutes())
                    .unwrap_or(0);
                let p_diff = r
                    .pred_dt
                    .map(|t| t.signed_duration_since(now).num_minutes())
                    .unwrap_or(s_diff);
                // Keep if scheduled or predicted is strictly greater than -5 mins
                s_diff > -5 || p_diff > -5
            })
            .collect()
    };

    let rows_kenmore = filter_rows(rows_kenmore);
    let rows_brookline_ave = filter_rows(rows_brookline_ave);
    let rows_pearl = filter_rows(rows_pearl);
    let rows_high = filter_rows(rows_high);
    let rows_copley = filter_rows(rows_copley);
    let rows_brookline = filter_rows(rows_brookline);

    // 2. Show Schedule
    let route60_stops = vec![
        format_stop_data("Kenmore (outbound)", &rows_kenmore, now),
        format_stop_data("Brookline Ave @ Fullerton (outbound)", &rows_brookline_ave, now),
        format_stop_data("Pearl St @ Brookline Village (outbound)", &rows_pearl, now),
        format_stop_data("High St @ Highland Rd (inbound)", &rows_high, now),
    ];
    print_stops_grid("Route 60:", route60_stops);

    let green_line_stops = vec![
        format_stop_data("Copley (to Riverside)", &rows_copley, now),
        format_stop_data("Brookline Village (to Kenmore)", &rows_brookline, now),
    ];
    print_stops_grid("Green Line D:", green_line_stops);

    Ok(())
}

async fn get_schedule_and_predictions(
    client: &Client,
    stop: &StopConfig,
    now: DateTime<Local>,
) -> Result<Vec<RowData>, Box<dyn Error>> {
    // Look back 30 mins to catch delayed trips
    let lookback_time = now - Duration::minutes(30);
    let sched_url = format!("{}/schedules", BASE_URL);
    let sched_params = [
        ("filter[stop]", stop.stop_id.to_string()),
        ("filter[route]", stop.route_id.to_string()),
        ("filter[direction_id]", stop.direction_id.to_string()),
        ("sort", "arrival_time".to_string()),
        (
            "filter[min_time]",
            lookback_time.format("%H:%M").to_string(),
        ),
        ("page[limit]", "20".to_string()), // Request more to ensure we have enough after filtering
    ];

    let sched_resp = client
        .get(&sched_url)
        .header("accept", "application/vnd.api+json")
        .query(&sched_params)
        .send()
        .await?;

    // Check for rate limiting
    if sched_resp.status().as_u16() == 429 {
        return Err("Rate limited".into());
    }

    let sched_text = sched_resp.text().await?;

    // Check for errors in the text response manually or just try to parse
    let sched_resp: ApiResponse<Resource<ScheduleAttributes, ScheduleRelationships>> =
        match serde_json::from_str(&sched_text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to parse Sched JSON: {}", e);
                eprintln!("Raw Body: {}", sched_text);
                return Err(Box::new(e));
            }
        };

    // 2. Fetch Predictions
    let pred_url = format!("{}/predictions", BASE_URL);
    let pred_params = [
        ("filter[stop]", stop.stop_id.to_string()),
        ("filter[route]", stop.route_id.to_string()),
        ("filter[direction_id]", stop.direction_id.to_string()),
        ("sort", "arrival_time".to_string()),
        ("page[limit]", "3".to_string()),
    ];

    let pred_resp = client
        .get(&pred_url)
        .header("accept", "application/vnd.api+json")
        .query(&pred_params)
        .send()
        .await?;

    // Check for rate limiting
    if pred_resp.status().as_u16() == 429 {
        return Err("Rate limited".into());
    }

    let pred_text = pred_resp.text().await?;

    let pred_resp: ApiResponse<Resource<PredictionAttributes, PredictionRelationships>> =
        match serde_json::from_str(&pred_text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to parse Pred JSON: {}", e);
                eprintln!("Raw Body: {}", pred_text);
                return Err(Box::new(e));
            }
        };

    // Map predictions by trip_id
    let mut predictions_map = HashMap::new();
    for p in pred_resp.data {
        predictions_map.insert(p.relationships.trip.data.id, p.attributes);
    }

    let mut results = Vec::new();

    for s in sched_resp.data {
        let trip_id = s.relationships.trip.data.id;

        let sched_time_str = if stop.is_origin {
            s.attributes.departure_time
        } else {
            s.attributes.arrival_time.or(s.attributes.departure_time)
        };

        let sched_dt = parse_time(sched_time_str);

        let pred_attr = predictions_map.get(&trip_id);
        let pred_dt = if let Some(p) = pred_attr {
            let pred_time_str = if stop.is_origin {
                p.departure_time.clone()
            } else {
                p.arrival_time.clone().or(p.departure_time.clone())
            };
            parse_time(pred_time_str)
        } else {
            None
        };

        results.push(RowData { sched_dt, pred_dt });
    }

    // Sort by time (use prediction if available, otherwise scheduled)
    results.sort_by_key(|r| {
        r.pred_dt
            .or(r.sched_dt)
            .unwrap_or_else(|| now + Duration::days(1))
    });

    Ok(results)
}

fn parse_time(time_str: Option<String>) -> Option<DateTime<Local>> {
    if let Some(s) = time_str {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
            return Some(dt.with_timezone(&Local));
        }
    }
    None
}

fn format_time_compact(dt: DateTime<Local>, now: DateTime<Local>) -> String {
    let time_str = dt.format("%H:%M").to_string();
    let diff = dt.signed_duration_since(now).num_minutes();
    if diff.abs() < 1 {
        time_str
    } else if diff < 0 {
        format!("{} ({}m ago)", time_str, diff.abs())
    } else {
        format!("{} (in {}m)", time_str, diff)
    }
}

fn format_time_compact_with_seconds(dt: DateTime<Local>, now: DateTime<Local>, include_seconds: bool) -> String {
    let time_str = if include_seconds {
        dt.format("%H:%M:%S").to_string()
    } else {
        dt.format("%H:%M").to_string()
    };
    let diff = dt.signed_duration_since(now).num_minutes();
    if diff.abs() < 1 {
        time_str
    } else if diff < 0 {
        format!("{} ({}m ago)", time_str, diff.abs())
    } else {
        format!("{} (in {}m)", time_str, diff)
    }
}

fn display_width(s: &str) -> usize {
    s.chars().map(|c| {
        match c {
            '🟢' | '📅' => 2,
            _ => 1,
        }
    }).sum()
}

fn pad_to_width(s: &str, target_width: usize) -> String {
    let current_width = display_width(s);
    if current_width >= target_width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target_width - current_width))
    }
}

struct StopDisplay {
    name: String,
    times: Vec<String>,
}

fn format_stop_data(stop_name: &str, rows: &[RowData], now: DateTime<Local>) -> StopDisplay {
    let mut times = Vec::new();

    if rows.is_empty() {
        times.push("No upcoming trips".to_string());
        return StopDisplay {
            name: stop_name.to_string(),
            times,
        };
    }

    let mut count = 0;
    let first_live_index = rows.iter().position(|r| r.pred_dt.is_some());

    for (idx, row) in rows.iter().enumerate() {
        if count >= 3 {
            break;
        }

        let time_str = match (row.sched_dt, row.pred_dt) {
            (_, Some(pred)) => {
                // Include seconds only for first live departure
                let include_seconds = first_live_index == Some(idx);
                format!("🟢 {}", format_time_compact_with_seconds(pred, now, include_seconds))
            }
            (Some(sched), None) => {
                format!("📅 {}", format_time_compact(sched, now))
            }
            (None, None) => continue,
        };

        times.push(time_str);
        count += 1;
    }

    if times.is_empty() {
        times.push("No upcoming trips".to_string());
    }

    StopDisplay {
        name: stop_name.to_string(),
        times,
    }
}

fn print_stops_grid(title: &str, stops: Vec<StopDisplay>) {
    println!("{}", title);
    println!();

    // Find max times count
    let max_times = stops.iter().map(|s| s.times.len()).max().unwrap_or(0);
    let col_width = 32;

    // Print stop names (may wrap to multiple lines)
    let max_name_lines = 3;
    for line_idx in 0..max_name_lines {
        for stop in &stops {
            let name_parts: Vec<&str> = stop.name.split_whitespace().collect();
            let mut current_line = String::new();
            let mut lines = Vec::new();

            for part in name_parts {
                if current_line.len() + part.len() + 1 <= col_width {
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(part);
                } else {
                    lines.push(current_line.clone());
                    current_line = part.to_string();
                }
            }
            if !current_line.is_empty() {
                lines.push(current_line);
            }

            let line_text = if line_idx < lines.len() {
                &lines[line_idx]
            } else {
                ""
            };

            print!("{}  ", pad_to_width(line_text, col_width));
        }
        println!();
    }

    println!();

    // Print times
    for time_idx in 0..max_times {
        for stop in &stops {
            let time_text = if time_idx < stop.times.len() {
                &stop.times[time_idx]
            } else {
                ""
            };
            print!("{}  ", pad_to_width(time_text, col_width));
        }
        println!();
    }

    println!();
}
