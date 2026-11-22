import { test } from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import { NexoraClient, NexoraSDKError } from "../src/index.js";

function mockServer(handler) {
  const server = http.createServer(handler);
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

function address(server) {
  return `http://127.0.0.1:${server.address().port}`;
}

test("info returns JSON", async () => {
  const server = await mockServer((req, res) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ engine: "nexora" }));
  });
  try {
    const client = new NexoraClient({ baseUrl: address(server) });
    assert.deepEqual(await client.info(), { engine: "nexora" });
  } finally { server.close(); }
});

test("search posts query sources and limit", async () => {
  let body = null;
  const server = await mockServer((req, res) => {
    let raw = "";
    req.on("data", (chunk) => { raw += chunk; });
    req.on("end", () => {
      body = JSON.parse(raw);
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ results: [] }));
    });
  });
  try {
    const client = new NexoraClient({ baseUrl: address(server) });
    await client.search("song", ["local-media"], 7);
    assert.deepEqual(body, { query: "song", sources: ["local-media"], limit: 7 });
  } finally { server.close(); }
});

test("createSession uses default output", async () => {
  let body = null;
  const server = await mockServer((req, res) => {
    let raw = "";
    req.on("data", (chunk) => { raw += chunk; });
    req.on("end", () => {
      body = JSON.parse(raw);
      res.writeHead(201, { "content-type": "application/json" });
      res.end(JSON.stringify({ session_id: "nx_session_1" }));
    });
  });
  try {
    const client = new NexoraClient({ baseUrl: address(server) });
    const created = await client.createSession();
    assert.equal(created.session_id, "nx_session_1");
    assert.deepEqual(body.output, { sample_rate: 48000, channels: 2, codec: "opus" });
  } finally { server.close(); }
});

test("api key sets bearer auth", async () => {
  let auth = null;
  const server = await mockServer((req, res) => {
    auth = req.headers.authorization;
    res.writeHead(200, { "content-type": "application/json" });
    res.end("{}");
  });
  try {
    const client = new NexoraClient({ baseUrl: address(server), apiKey: "secret" });
    await client.info();
    assert.equal(auth, "Bearer secret");
  } finally { server.close(); }
});

test("http error maps to NexoraSDKError", async () => {
  const server = await mockServer((req, res) => {
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: { code: "NOT_FOUND", message: "missing" } }));
  });
  try {
    const client = new NexoraClient({ baseUrl: address(server) });
    await assert.rejects(() => client.getSession("missing"), (error) => {
      assert.ok(error instanceof NexoraSDKError);
      assert.equal(error.status, 404);
      assert.equal(error.code, "NOT_FOUND");
      return true;
    });
  } finally { server.close(); }
});
