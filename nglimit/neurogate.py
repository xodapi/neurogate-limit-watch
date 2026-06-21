from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import json
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


DEFAULT_API_BASE = "https://api.neurogate.space"


@dataclass(frozen=True)
class MetricSummary:
    used: float
    limit: float
    remaining: float
    percent: float


@dataclass(frozen=True)
class WindowSummary:
    key: str
    label: str
    credits: MetricSummary | None
    requests: MetricSummary | None
    reset_at: str | None
    reset_in_seconds: int | None
    level: str


WINDOWS = [
    ("5h", "5Hours", "window5HoursEndsAt"),
    ("24h", "24Hours", "window24HoursEndsAt"),
    ("7d", "7Days", "window7DaysEndsAt"),
    ("30d", "30Days", "window30DaysEndsAt"),
]


class NeuroGateError(RuntimeError):
    pass


def fetch_me(api_key: str, api_base: str = DEFAULT_API_BASE, timeout: float = 10.0) -> dict[str, Any]:
    if not api_key:
        raise NeuroGateError("NEUROGATE_API_KEY is required unless --mock is used")

    url = api_base.rstrip("/") + "/v1/me"
    request = Request(
        url,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Accept": "application/json",
            "User-Agent": "neurogate-limit-watch/0.1",
        },
    )
    try:
        with urlopen(request, timeout=timeout) as response:
            raw = response.read().decode("utf-8")
    except HTTPError as exc:
        raise NeuroGateError(f"NeuroGate /v1/me returned HTTP {exc.code}") from exc
    except URLError as exc:
        raise NeuroGateError(f"cannot reach NeuroGate API: {exc.reason}") from exc
    except TimeoutError as exc:
        raise NeuroGateError("NeuroGate API request timed out") from exc

    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise NeuroGateError("NeuroGate /v1/me returned invalid JSON") from exc
    if not isinstance(parsed, dict):
        raise NeuroGateError("NeuroGate /v1/me returned a non-object JSON payload")
    return parsed


def load_mock(path: str) -> dict[str, Any]:
    with open(path, "r", encoding="utf-8") as fh:
        parsed = json.load(fh)
    if not isinstance(parsed, dict):
        raise NeuroGateError("mock payload must be a JSON object")
    return parsed


def summarize_me(payload: dict[str, Any], now: datetime | None = None) -> list[WindowSummary]:
    rows = extract_usage_rows(payload)
    now = now or datetime.now(timezone.utc)
    summaries: list[WindowSummary] = []
    for key, suffix, reset_field in WINDOWS:
        credits = summarize_metric(rows, f"credits{suffix}", f"creditLimit{suffix}")
        requests = summarize_metric(rows, f"requests{suffix}", f"requestLimit{suffix}")
        reset_at = first_value(rows, reset_field)
        reset_iso, reset_seconds = parse_reset(reset_at, now)
        if credits is None and requests is None and reset_iso is None:
            continue
        summaries.append(
            WindowSummary(
                key=key,
                label=key,
                credits=credits,
                requests=requests,
                reset_at=reset_iso,
                reset_in_seconds=reset_seconds,
                level=window_level(credits, requests),
            )
        )
    return summaries


def extract_usage_rows(payload: dict[str, Any]) -> list[dict[str, Any]]:
    candidates = [
        payload.get("usage"),
        payload.get("data", {}).get("usage") if isinstance(payload.get("data"), dict) else None,
        payload,
    ]
    for candidate in candidates:
        if isinstance(candidate, dict) and isinstance(candidate.get("rows"), list):
            return [row for row in candidate["rows"] if isinstance(row, dict)]
    return []


def summarize_metric(rows: list[dict[str, Any]], used_field: str, limit_field: str) -> MetricSummary | None:
    used_total = 0.0
    limit_values: list[float] = []
    seen = False
    for row in rows:
        used = to_number(row.get(used_field))
        limit = to_number(row.get(limit_field))
        if used is None and limit is None:
            continue
        seen = True
        used_total += used or 0.0
        if limit is not None and limit > 0 and limit not in limit_values:
            limit_values.append(limit)
    limit_total = sum(limit_values)
    if not seen or limit_total <= 0:
        return None
    remaining = max(limit_total - used_total, 0.0)
    percent = min(max((used_total / limit_total) * 100.0, 0.0), 999.0)
    return MetricSummary(used=used_total, limit=limit_total, remaining=remaining, percent=percent)


def first_value(rows: list[dict[str, Any]], field: str) -> Any:
    for row in rows:
        value = row.get(field)
        if value not in (None, ""):
            return value
    return None


def to_number(value: Any) -> float | None:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return None
    return None


def parse_reset(value: Any, now: datetime) -> tuple[str | None, int | None]:
    if value in (None, ""):
        return None, None
    dt: datetime | None = None
    if isinstance(value, (int, float)):
        dt = datetime.fromtimestamp(float(value), tz=timezone.utc)
    elif isinstance(value, str):
        raw = value.strip()
        try:
            if raw.isdigit():
                dt = datetime.fromtimestamp(float(raw), tz=timezone.utc)
            else:
                dt = datetime.fromisoformat(raw.replace("Z", "+00:00"))
                if dt.tzinfo is None:
                    dt = dt.replace(tzinfo=timezone.utc)
        except ValueError:
            return raw, None
    if dt is None:
        return str(value), None
    seconds = max(int((dt - now).total_seconds()), 0)
    return dt.astimezone(timezone.utc).isoformat().replace("+00:00", "Z"), seconds


def window_level(*metrics: MetricSummary | None) -> str:
    percents = [metric.percent for metric in metrics if metric is not None]
    if not percents:
        return "unknown"
    peak = max(percents)
    if peak >= 90:
        return "danger"
    if peak >= 75:
        return "warning"
    return "ok"


def summary_to_json(windows: list[WindowSummary], abtop_status: dict[str, Any] | None = None) -> dict[str, Any]:
    return {
        "source": "neurogate",
        "windows": [window_to_json(window) for window in windows],
        "abtop": abtop_status,
    }


def window_to_json(window: WindowSummary) -> dict[str, Any]:
    return {
        "window": window.key,
        "level": window.level,
        "reset_at": window.reset_at,
        "reset_in_seconds": window.reset_in_seconds,
        "credits": metric_to_json(window.credits),
        "requests": metric_to_json(window.requests),
    }


def metric_to_json(metric: MetricSummary | None) -> dict[str, float] | None:
    if metric is None:
        return None
    return {
        "used": metric.used,
        "limit": metric.limit,
        "remaining": metric.remaining,
        "percent": metric.percent,
    }
