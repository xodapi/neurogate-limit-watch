from __future__ import annotations

import json
import os
import subprocess
from typing import Any


def read_abtop_status(timeout: float = 5.0) -> dict[str, Any] | None:
    binary = os.environ.get("ABTOP_BIN", "abtop")
    try:
        result = subprocess.run(
            [binary, "--status-json"],
            check=True,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except (FileNotFoundError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return None
    try:
        parsed = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, dict) else None

