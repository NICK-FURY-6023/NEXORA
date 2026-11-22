export class NexoraSDKError extends Error {
  constructor(status, body) {
    const message = body?.error?.message || `HTTP ${status}`;
    super(message);
    this.name = "NexoraSDKError";
    this.status = status;
    this.code = body?.error?.code || "UNKNOWN";
    this.body = body;
  }
}

const DEFAULT_OUTPUT = { sample_rate: 48000, channels: 2, codec: "opus" };

export class NexoraClient {
  constructor({ baseUrl = "http://127.0.0.1:8080", apiKey = null } = {}) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.apiKey = apiKey;
  }

  _headers(body) {
    const headers = { accept: "application/json" };
    if (body !== undefined) headers["content-type"] = "application/json";
    if (this.apiKey) headers.authorization = `Bearer ${this.apiKey}`;
    return headers;
  }

  async _request(method, path, body) {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: this._headers(body),
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await response.text();
    let parsed = null;
    if (text) {
      try { parsed = JSON.parse(text); } catch { parsed = { raw: text }; }
    }
    if (!response.ok) throw new NexoraSDKError(response.status, parsed);
    return parsed;
  }

  info() { return this._request("GET", "/api/v1/info"); }
  sources() { return this._request("GET", "/api/v1/sources"); }

  search(query, sources = null, limit = 10) {
    return this._request("POST", "/api/v1/search", { query, sources, limit });
  }

  createSession(output = DEFAULT_OUTPUT) {
    return this._request("POST", "/api/v1/sessions", {
      application: "nodejs-sdk", version: "1.0.0", output: output || DEFAULT_OUTPUT,
    });
  }

  getSession(id) { return this._request("GET", `/api/v1/sessions/${id}`); }
  play(id, trackId, startPositionMs = 0) {
    return this._request("POST", `/api/v1/sessions/${id}/play`, { track_id: trackId, start_position_ms: startPositionMs });
  }
  pause(id) { return this._request("POST", `/api/v1/sessions/${id}/pause`); }
  resume(id) { return this._request("POST", `/api/v1/sessions/${id}/resume`); }
  stop(id) { return this._request("POST", `/api/v1/sessions/${id}/stop`); }
  seek(id, positionMs) { return this._request("POST", `/api/v1/sessions/${id}/seek`, { position_ms: positionMs }); }
  setVolume(id, volume) { return this._request("POST", `/api/v1/sessions/${id}/volume`, { volume }); }
  queue(id) { return this._request("GET", `/api/v1/sessions/${id}/queue`); }
  enqueue(id, trackId) { return this._request("POST", `/api/v1/sessions/${id}/queue`, { track_id: trackId }); }

  websocketUrl(id) {
    const base = this.baseUrl.replace(/^http/, "ws");
    return `${base}/ws/v1/sessions/${id}`;
  }

  subscribe(id, onEvent) {
    const socket = new WebSocket(this.websocketUrl(id));
    socket.onmessage = (event) => {
      try { onEvent(JSON.parse(event.data)); } catch { onEvent(event.data); }
    };
    return socket;
  }
}
