import io
import json
import unittest
import urllib.error
from unittest.mock import patch

from nexora_sdk import NexoraClient, NexoraSDKError


class FakeResponse:
    def __init__(self, payload, status=200):
        self.payload = json.dumps(payload).encode("utf-8")
        self.status = status

    def read(self):
        return self.payload

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        return False


class NexoraClientTests(unittest.TestCase):
    def test_info_returns_json(self):
        payload = {"engine": "nexora", "api_version": "v1"}

        with patch("urllib.request.urlopen", return_value=FakeResponse(payload)) as urlopen:
            result = NexoraClient().info()

        request = urlopen.call_args.args[0]
        self.assertEqual(result, payload)
        self.assertEqual(request.get_method(), "GET")
        self.assertEqual(request.full_url, "http://127.0.0.1:8080/api/v1/info")

    def test_search_posts_query_sources_and_limit(self):
        with patch("urllib.request.urlopen", return_value=FakeResponse({"results": []})) as urlopen:
            result = NexoraClient().search("jazz", ["local", "radio"], 5)

        request = urlopen.call_args.args[0]
        self.assertEqual(result, {"results": []})
        self.assertEqual(request.get_method(), "POST")
        self.assertEqual(
            json.loads(request.data.decode("utf-8")),
            {"query": "jazz", "sources": ["local", "radio"], "limit": 5},
        )

    def test_create_session_uses_default_output(self):
        with patch(
            "urllib.request.urlopen",
            return_value=FakeResponse({"session_id": "session-1"}),
        ) as urlopen:
            result = NexoraClient().create_session()

        request = urlopen.call_args.args[0]
        body = json.loads(request.data.decode("utf-8"))
        self.assertEqual(result, {"session_id": "session-1"})
        self.assertEqual(request.get_method(), "POST")
        self.assertEqual(
            body,
            {
                "application": "python-sdk",
                "version": "1.0.0",
                "output": {"sample_rate": 48000, "channels": 2, "codec": "opus"},
            },
        )

    def test_play_posts_track_and_start_position(self):
        with patch(
            "urllib.request.urlopen",
            return_value=FakeResponse({"status": "playing"}),
        ) as urlopen:
            result = NexoraClient().play("session-1", "track-1", 250)

        request = urlopen.call_args.args[0]
        self.assertEqual(result, {"status": "playing"})
        self.assertEqual(request.get_method(), "POST")
        self.assertEqual(
            request.full_url,
            "http://127.0.0.1:8080/api/v1/sessions/session-1/play",
        )
        self.assertEqual(
            json.loads(request.data.decode("utf-8")),
            {"track_id": "track-1", "start_position_ms": 250},
        )

    def test_api_key_sets_bearer_auth_header(self):
        with patch("urllib.request.urlopen", return_value=FakeResponse({})) as urlopen:
            NexoraClient(api_key="secret").info()

        request = urlopen.call_args.args[0]
        self.assertEqual(request.get_header("Authorization"), "Bearer secret")

    def test_http_error_maps_to_nexora_sdk_error(self):
        body = {"error": {"code": "invalid_request", "message": "Bad request"}}
        http_error = urllib.error.HTTPError(
            "http://127.0.0.1:8080/api/v1/search",
            400,
            "Bad Request",
            {},
            io.BytesIO(json.dumps(body).encode("utf-8")),
        )

        with patch("urllib.request.urlopen", side_effect=http_error):
            with self.assertRaises(NexoraSDKError) as raised:
                NexoraClient().search("query")

        self.assertEqual(raised.exception.status_code, 400)
        self.assertEqual(raised.exception.error_body, body)
        self.assertIn("Bad request", str(raised.exception))


if __name__ == "__main__":
    unittest.main()
