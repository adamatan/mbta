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
    vehicle: Option<OptionalDataWrapper>,
    stop: Option<DataWrapper>,
}

#[derive(Debug, Deserialize)]
struct OptionalDataWrapper {
    data: Option<IdWrapper>,
}

#[derive(Debug, Deserialize)]
struct IncludedResource {
    #[serde(rename = "type")]
    resource_type: String,
    id: String,
    #[serde(default)]
    relationships: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct RouteStopsResponse {
    data: Vec<RouteStop>,
}

#[derive(Debug, Deserialize)]
struct RouteStop {
    id: String,
}

#[derive(Debug, Deserialize)]
struct PredictionApiResponse {
    data: Vec<Resource<PredictionAttributes, PredictionRelationships>>,
    #[serde(default)]
    included: Vec<IncludedResource>,
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
    stops_away: Option<i32>,
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

    // Filter rows > 5 mins ago, drop past schedule-only when live data exists
    let filter_rows = |rows: Vec<RowData>| -> Vec<RowData> {
        let filtered: Vec<RowData> = rows.into_iter()
            .filter(|r| {
                let s_diff = r
                    .sched_dt
                    .map(|t| t.signed_duration_since(now).num_minutes())
                    .unwrap_or(0);
                let p_diff = r
                    .pred_dt
                    .map(|t| t.signed_duration_since(now).num_minutes())
                    .unwrap_or(s_diff);
                s_diff > -5 || p_diff > -5
            })
            .collect();
        let has_live = filtered.iter().any(|r| r.pred_dt.is_some());
        if has_live {
            filtered.into_iter()
                .filter(|r| r.pred_dt.is_some() || r.sched_dt.map(|t| t > now).unwrap_or(false))
                .collect()
        } else {
            filtered
        }
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

    // 2. Fetch Predictions (with vehicle data)
    let pred_url = format!("{}/predictions", BASE_URL);
    let pred_params = [
        ("filter[stop]", stop.stop_id.to_string()),
        ("filter[route]", stop.route_id.to_string()),
        ("filter[direction_id]", stop.direction_id.to_string()),
        ("sort", "arrival_time".to_string()),
        ("page[limit]", "3".to_string()),
        ("include", "vehicle,stop".to_string()),
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

    let pred_resp: PredictionApiResponse =
        match serde_json::from_str(&pred_text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to parse Pred JSON: {}", e);
                eprintln!("Raw Body: {}", pred_text);
                return Err(Box::new(e));
            }
        };

    // Extract vehicle current stop IDs and build child->parent stop map
    let mut vehicle_stop_ids: HashMap<String, String> = HashMap::new(); // vehicle_id -> child stop ID
    let mut stop_parent_map: HashMap<String, String> = HashMap::new(); // child stop ID -> parent station ID
    for inc in &pred_resp.included {
        if inc.resource_type == "vehicle" {
            if let Some(stop_data) = inc.relationships.get("stop")
                .and_then(|s| s.get("data"))
                .and_then(|d| d.get("id"))
                .and_then(|id| id.as_str()) {
                vehicle_stop_ids.insert(inc.id.clone(), stop_data.to_string());
            }
        } else if inc.resource_type == "stop" {
            let parent_id = inc.relationships.get("parent_station")
                .and_then(|ps| ps.get("data"))
                .and_then(|d| d.get("id"))
                .and_then(|id| id.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| inc.id.clone());
            stop_parent_map.insert(inc.id.clone(), parent_id);
        }
    }
    // Collect all stop IDs we need to resolve (vehicle stops + prediction stops)
    let pred_stop_ids: Vec<String> = pred_resp.data.iter()
        .filter_map(|p| p.relationships.stop.as_ref().map(|s| s.data.id.clone()))
        .collect();
    let all_stop_ids: Vec<String> = vehicle_stop_ids.values().cloned()
        .chain(pred_stop_ids.into_iter())
        .collect();
    // Batch-resolve unknown child stop IDs to their parent stations
    let unknown_ids: Vec<String> = all_stop_ids.iter()
        .filter(|id| !stop_parent_map.contains_key(*id))
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    if !unknown_ids.is_empty() {
        let ids_param = unknown_ids.join(",");
        if let Ok(resp) = client
            .get(&format!("{}/stops", BASE_URL))
            .header("accept", "application/vnd.api+json")
            .query(&[("filter[id]", &ids_param)])
            .send()
            .await
        {
            if let Ok(text) = resp.text().await {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
                        for item in data {
                            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let parent_id = item.get("relationships")
                                .and_then(|r| r.get("parent_station"))
                                .and_then(|ps| ps.get("data"))
                                .and_then(|d| d.get("id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(id);
                            stop_parent_map.insert(id.to_string(), parent_id.to_string());
                        }
                    }
                }
            }
        }
    }
    let to_parent = |id: &str| -> String {
        stop_parent_map.get(id).cloned().unwrap_or_else(|| id.to_string())
    };

    // Fetch route stops list for counting stops between vehicle and target
    let route_stop_ids: Vec<String> = {
        let route_stops_url = format!("{}/stops", BASE_URL);
        let route_stops_params = [
            ("filter[route]", stop.route_id.to_string()),
            ("filter[direction_id]", stop.direction_id.to_string()),
        ];
        match client
            .get(&route_stops_url)
            .header("accept", "application/vnd.api+json")
            .query(&route_stops_params)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.text().await {
                    Ok(text) => serde_json::from_str::<RouteStopsResponse>(&text)
                        .map(|r| r.data.into_iter().map(|s| s.id).collect())
                        .unwrap_or_default(),
                    Err(_) => vec![],
                }
            }
            _ => vec![],
        }
    };

    // Map predictions by trip_id, with vehicle and stop info
    struct PredInfo {
        attrs: PredictionAttributes,
        vehicle_stop: Option<String>,
        pred_stop: Option<String>,
    }
    let mut predictions_map: HashMap<String, PredInfo> = HashMap::new();
    for p in pred_resp.data {
        let vehicle_current_stop = p.relationships.vehicle
            .as_ref()
            .and_then(|v| v.data.as_ref())
            .and_then(|d| vehicle_stop_ids.get(&d.id).cloned());
        let pred_stop = p.relationships.stop
            .as_ref()
            .map(|s| s.data.id.clone());
        predictions_map.insert(p.relationships.trip.data.id, PredInfo {
            attrs: p.attributes,
            vehicle_stop: vehicle_current_stop,
            pred_stop,
        });
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

        let pred_entry = predictions_map.get(&trip_id);
        let (pred_dt, stops_away) = if let Some(info) = pred_entry {
            let pred_time_str = if stop.is_origin {
                info.attrs.departure_time.clone()
            } else {
                info.attrs.arrival_time.clone().or(info.attrs.departure_time.clone())
            };
            let dt = parse_time(pred_time_str);
            // Count actual stops between vehicle and target using route stops list
            let sa = match (&info.vehicle_stop, &info.pred_stop) {
                (Some(v_stop), Some(t_stop)) if !route_stop_ids.is_empty() => {
                    let v_parent = to_parent(v_stop);
                    let t_parent = to_parent(t_stop);
                    let v_idx = route_stop_ids.iter().position(|id| *id == v_parent);
                    let t_idx = route_stop_ids.iter().position(|id| *id == t_parent);
                    match (v_idx, t_idx) {
                        (Some(vi), Some(ti)) => {
                            let diff = (ti as i32 - vi as i32).unsigned_abs() as i32;
                            if diff > 0 && diff <= 20 { Some(diff) } else { None }
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            (dt, sa)
        } else {
            (None, None)
        };

        results.push(RowData { sched_dt, pred_dt, stops_away });
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
                let base = format!("🟢 {}", format_time_compact_with_seconds(pred, now, include_seconds));
                match row.stops_away {
                    Some(n) if n > 0 => format!("{} ({} stop{})", base, n, if n == 1 { "" } else { "s" }),
                    _ => base,
                }
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

    // Find max times count
    let max_times = stops.iter().map(|s| s.times.len()).max().unwrap_or(0);
    let col_width = 32;

    // Pre-compute wrapped names for all stops
    let wrapped_names: Vec<Vec<String>> = stops.iter().map(|stop| {
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
        lines
    }).collect();

    // Find actual max name lines needed
    let max_name_lines = wrapped_names.iter().map(|lines| lines.len()).max().unwrap_or(1);

    // Print stop names (may wrap to multiple lines)
    for line_idx in 0..max_name_lines {
        for lines in &wrapped_names {
            let line_text = if line_idx < lines.len() {
                &lines[line_idx]
            } else {
                ""
            };

            print!("{}  ", pad_to_width(line_text, col_width));
        }
        println!();
    }

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
