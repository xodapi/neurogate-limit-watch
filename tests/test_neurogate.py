from __future__ import annotations

from datetime import datetime, timezone
import json
import unittest

from nglimit.neurogate import load_mock, summarize_me, summary_to_json


class NeuroGateSummaryTests(unittest.TestCase):
    def test_summarizes_credit_and_request_windows(self) -> None:
        payload = load_mock("tests/fixtures/me.json")
        now = datetime(2026, 6, 21, 16, 0, tzinfo=timezone.utc)

        windows = summarize_me(payload, now=now)

        self.assertEqual([window.key for window in windows], ["5h", "24h", "7d", "30d"])
        five_hour = windows[0]
        self.assertEqual(five_hour.level, "warning")
        self.assertAlmostEqual(five_hour.credits.percent, 78.0)
        self.assertEqual(five_hour.reset_in_seconds, 9000)

    def test_repeated_limit_rows_do_not_double_count_cap(self) -> None:
        payload = {
            "usage": {
                "rows": [
                    {"credits5Hours": 10, "creditLimit5Hours": 50},
                    {"credits5Hours": 20, "creditLimit5Hours": 50},
                ]
            }
        }

        windows = summarize_me(payload, now=datetime(2026, 6, 21, 16, 0, tzinfo=timezone.utc))

        self.assertEqual(windows[0].credits.used, 30)
        self.assertEqual(windows[0].credits.limit, 50)
        self.assertAlmostEqual(windows[0].credits.percent, 60.0)

    def test_json_summary_has_no_account_identity(self) -> None:
        payload = load_mock("tests/fixtures/me.json")
        windows = summarize_me(payload, now=datetime(2026, 6, 21, 16, 0, tzinfo=timezone.utc))

        encoded = json.dumps(summary_to_json(windows))

        self.assertIn('"source": "neurogate"', encoded)
        self.assertNotIn("usr_demo", encoded)
        self.assertNotIn("api", encoded.lower())


if __name__ == "__main__":
    unittest.main()
