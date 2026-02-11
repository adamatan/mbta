# MBTA Schedule Fetcher

A Rust application that provides real-time transit information for MBTA Route 60 and Green Line D stops.

## Features

- Fetches live data from the MBTA V3 API
- Compares scheduled times with real-time predictions
- Shows next 3 trips per stop
- Filters out trips that departed more than 5 minutes ago
- Displays live predictions (🟢) when available, scheduled times (📅) otherwise

## Usage

### Prerequisites
- **Rust** and **Cargo** installed

### Run
```bash
cargo run
```

### Install
Install the `b60` command globally:
```bash
cargo install --path .
b60
```

## Monitored Stops

### Route 60
- **Kenmore (outbound)** - Route 60 towards Chestnut Hill
- **Pearl St @ Brookline Village (outbound)** - Route 60 towards Chestnut Hill
- **High St @ Highland Rd (inbound)** - Route 60 towards Kenmore

### Green Line D
- **Copley (to Riverside)** - Green Line D westbound
- **Brookline Village (to Kenmore)** - Green Line D eastbound

## API Technical Details

### Base URL
```
https://api-v3.mbta.com
```

### Route IDs
- **Route 60:** `60`
- **Green Line D:** `Green-D`

### Stop IDs and Directions

#### Route 60
| Stop Name | Stop ID | Direction | Direction ID | Description |
|-----------|---------|-----------|--------------|-------------|
| Kenmore | `place-kencl` | Outbound | `0` | Origin stop, towards Chestnut Hill |
| Pearl St @ Brookline Village | `11366` | Outbound | `0` | Towards Chestnut Hill |
| High St @ Highland Rd | `1553` | Inbound | `1` | Towards Kenmore |

#### Green Line D
| Stop Name | Stop ID | Direction | Direction ID | Description |
|-----------|---------|-----------|--------------|-------------|
| Copley | `place-coecl` | West | `0` | Towards Riverside |
| Brookline Village | `place-bvmnl` | East | `1` | Towards Union Square/Kenmore |

### API Endpoints

#### 1. Schedules Endpoint
Used for static timetables.

**URL:** `/schedules`

**Parameters:**
- `filter[route]` - Route ID (e.g., `60` or `Green-D`)
- `filter[stop]` - Stop ID
- `filter[direction_id]` - Direction (0 or 1)
- `filter[min_time]` - Minimum time in HH:MM format
- `sort` - Sort field (typically `arrival_time`)
- `page[limit]` - Number of results to return

**Example - Route 60 Kenmore:**
```bash
curl -s "https://api-v3.mbta.com/schedules?filter%5Broute%5D=60&filter%5Bstop%5D=place-kencl&filter%5Bdirection_id%5D=0&filter%5Bmin_time%5D=14:00&sort=arrival_time&page%5Blimit%5D=3" | jq -r '.data[] | "\(.attributes.departure_time)"'
```

**Example - Green Line D Copley:**
```bash
curl -s "https://api-v3.mbta.com/schedules?filter%5Broute%5D=Green-D&filter%5Bstop%5D=place-coecl&filter%5Bdirection_id%5D=0&filter%5Bmin_time%5D=14:00&sort=arrival_time&page%5Blimit%5D=3" | jq -r '.data[] | "\(.attributes.departure_time)"'
```

#### 2. Predictions Endpoint
Used for real-time vehicle locations and predictions.

**URL:** `/predictions`

**Parameters:**
- `filter[route]` - Route ID
- `filter[stop]` - Stop ID
- `filter[direction_id]` - Direction (0 or 1)
- `sort` - Sort field (typically `arrival_time`)
- `page[limit]` - Number of results to return

**Example - Route 60 Kenmore:**
```bash
curl -s "https://api-v3.mbta.com/predictions?filter%5Broute%5D=60&filter%5Bstop%5D=place-kencl&filter%5Bdirection_id%5D=0&page%5Blimit%5D=3" | jq -r '.data[] | "\(.attributes.departure_time)"'
```

**Example - Green Line D Brookline Village:**
```bash
curl -s "https://api-v3.mbta.com/predictions?filter%5Broute%5D=Green-D&filter%5Bstop%5D=place-bvmnl&filter%5Bdirection_id%5D=1&page%5Blimit%5D=3" | jq -r '.data[] | "\(.attributes.departure_time)"'
```

### Finding Stop and Route Information

#### Get Route Details
```bash
curl -s "https://api-v3.mbta.com/routes/Green-D" | jq '.'
```

#### Verify Stop ID
```bash
curl -s "https://api-v3.mbta.com/stops/place-coecl" | jq '{id: .data.id, name: .data.attributes.name}'
```

#### List All Routes
```bash
curl -s "https://api-v3.mbta.com/routes" | jq -r '.data[] | "\(.id): \(.attributes.long_name)"' | grep -i "green"
```

### Implementation Notes

- **Lookback Window:** The application looks back 30 minutes for schedules to catch delayed trips still in the prediction feed
- **Sorting:** Results are sorted by time (prediction if available, otherwise scheduled) since the API doesn't always return chronologically ordered results
- **Filtering:** Trips that departed more than 5 minutes ago are filtered out
- **Origin Stops:** For origin stops (`is_origin: true`), we use `departure_time`; for other stops, we use `arrival_time` (with fallback to `departure_time`)
- **Rate Limiting:** The application detects HTTP 429 responses and exits gracefully with a user-friendly message
- **Concurrent Fetching:** All stops are queried concurrently using `tokio::join!` for better performance

### API Documentation
Full MBTA API documentation: https://api-v3.mbta.com/docs/swagger/index.html