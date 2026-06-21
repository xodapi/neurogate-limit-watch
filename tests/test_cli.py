from __future__ import annotations

import unittest

from nglimit.cli import exit_code, format_agent


class CliTests(unittest.TestCase):
    def test_fail_on_warning_catches_warning_and_danger(self) -> None:
        status = {"windows": [{"level": "ok"}, {"level": "warning"}, {"level": "danger"}]}

        self.assertEqual(exit_code(status, "warning"), 2)
        self.assertEqual(exit_code(status, "danger"), 3)
        self.assertEqual(exit_code(status, "never"), 0)

    def test_format_agent_tolerates_missing_fields(self) -> None:
        rendered = format_agent({"agent_cli": "codex"})

        self.assertIn("codex", rendered)
        self.assertIn("ctx max n/a", rendered)


if __name__ == "__main__":
    unittest.main()
