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
    let (res_kenmore, res_pearl, res_high, res_copley, res_brookline) = tokio::join!(
        get_schedule_and_predictions(&client, &stop_kenmore, now),
        get_schedule_and_predictions(&client, &stop_pearl, now),
        get_schedule_and_predictions(&client, &stop_high, now),
        get_schedule_and_predictions(&client, &stop_copley, now),
        get_schedule_and_predictions(&client, &stop_brookline, now)
    );

    // Check for rate limiting first
    if res_kenmore.is_err() && res_kenmore.as_ref().unwrap_err().to_string() == "Rate limited"
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
    let rows_pearl = filter_rows(rows_pearl);
    let rows_high = filter_rows(rows_high);
    let rows_copley = filter_rows(rows_copley);
    let rows_brookline = filter_rows(rows_brookline);

    // 2. Show Schedule
    println!("Route 60:");
    print_stop_schedule("  Kenmore (outbound)", &rows_kenmore, now);
    println!();
    print_stop_schedule("  Pearl St @ Brookline Village (outbound)", &rows_pearl, now);
    println!();
    print_stop_schedule("  High St @ Highland Rd (inbound)", &rows_high, now);
    println!();
    println!("Green Line D:");
    print_stop_schedule("  Copley (to Riverside)", &rows_copley, now);
    println!();
    print_stop_schedule("  Brookline Village (to Kenmore)", &rows_brookline, now);

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
    let time_str = dt.format("%H:%M:%S").to_string();
    let diff = dt.signed_duration_since(now).num_minutes();
    if diff.abs() < 1 {
        time_str
    } else if diff < 0 {
        format!("{} ({}m ago)", time_str, diff.abs())
    } else {
        format!("{} (in {}m)", time_str, diff)
    }
}

fn print_stop_schedule(stop_name: &str, rows: &[RowData], now: DateTime<Local>) {
    if rows.is_empty() {
        println!("{:<45} No upcoming trips", stop_name);
        return;
    }

    let mut count = 0;
    for (idx, row) in rows.iter().enumerate() {
        if count >= 3 {
            break;
        }

        let time_str = match (row.sched_dt, row.pred_dt) {
            (_, Some(pred)) => {
                // If live data exists, show only live
                format!("🟢 {}", format_time_compact(pred, now))
            }
            (Some(sched), None) => {
                format!("📅 {}", format_time_compact(sched, now))
            }
            (None, None) => continue,
        };

        if idx == 0 {
            println!("{:<45} {}", stop_name, time_str);
        } else {
            println!("{:<45} {}", "", time_str);
        }
        count += 1;
    }

    if count == 0 {
        println!("{:<45} No upcoming trips", stop_name);
    }
}
