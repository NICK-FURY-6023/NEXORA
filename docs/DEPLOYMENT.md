# NEXORA Deployment

## Docker Compose

```bash
cp deploy/.env.example deploy/.env
# Set NEXORA_SERVER_PASSWORD in deploy/.env for non-local use.
docker compose -f deploy/docker-compose.yml up --build -d
```

Ports:

- `8080` — REST and WebSocket (`/health`, `/api/v1`, `/nexora/*`, `/ws/v1`)
- `9090` — internal gRPC (`nexora.v1.SessionService`)

The container runs `nexora-server --host 0.0.0.0`. Local media can be mounted read-only at `/app/data/media`; pass `--media-root /app/data/media` to the binary if using a custom entrypoint.

## Health check

```bash
curl -fsS http://127.0.0.1:8080/health
```

## Password

Provide `NEXORA_SERVER_PASSWORD` via `.env` or the container environment. When unset or empty, authorization is disabled for local development only. Never commit a real value.

## Production notes

- Terminate TLS with a reverse proxy; NEXORA itself exposes plain HTTP/gRPC inside the trusted network.
- Restrict access to port 9090; it is an internal service RPC.
- Use a read-only media mount and keep source credentials outside the image.
- Runtime metrics are available at `/api/v1/metrics`; Lavalink-compatible stats at `/nexora/stats`.

## Verification

After startup, check:

```bash
curl -fsS http://127.0.0.1:8080/nexora/info
curl -fsS http://127.0.0.1:8080/api/v1/metrics
```
