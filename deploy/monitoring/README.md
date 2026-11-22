# NEXORA monitoring examples

`prometheus.yml` and `alerts.yml` are illustrative examples. NEXORA currently returns JSON from `/api/v1/metrics` and `/v4/stats`; it does **not** expose Prometheus text format. A metrics exporter must be added before these files are used in production.

Runtime counters currently available in JSON:

- `nexora_active_sessions`
- `nexora_api_requests_total`
- `nexora_search_requests_total`
- `nexora_stream_bytes_total`
- `nexora_api_errors_total`
