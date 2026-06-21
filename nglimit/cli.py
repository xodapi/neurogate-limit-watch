from __future__ import annotations

import argparse
import json
import os
import sys
import time
from typing import Sequence

from . import __version__
from .abtop import read_abtop_status
from .neurogate import (
    DEFAULT_API_BASE,
    NeuroGateError,
    fetch_me,
    load_mock,
    summary_to_json,
    summarize_me,
)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="nglimit",
        description="Safe NeuroGate quota monitor for Codex/Droid workflows.",
    )
    parser.add_argument("--api-base", default=os.environ.get("NEUROGATE_API_BASE", DEFAULT_API_BASE))
    parser.add_argument("--api-key-env", default="NEUROGATE_API_KEY")
    parser.add_argument("--mock", help="Read a saved /v1/me JSON payload instead of calling NeuroGate")
    parser.add_argument("--json", action="store_true", help="Print machine-readable JSON")
    parser.add_argument("--with-abtop", action="store_true", help="Merge local abtop --status-json output if available")
    parser.add_argument("--watch", type=float, default=0.0, help="Poll every N seconds")
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    parser.add_argument(
        "--fail-on",
        choices=["never", "warning", "danger"],
        default="never",
        help="Return a non-zero exit code when a threshold is reached",
    )
    args = parser.parse_args(argv)

    try:
        while True:
            code = run_once(args)
            if args.watch <= 0:
                return code
            if args.fail_on != "never" and code != 0:
                return code
            time.sleep(args.watch)
    except KeyboardInterrupt:
        return 130


def run_once(args: argparse.Namespace) -> int:
    try:
        payload = load_mock(args.mock) if args.mock else fetch_me(os.environ.get(args.api_key_env, ""), args.api_base)
        windows = summarize_me(payload)
    except (OSError, NeuroGateError) as exc:
        print(f"nglimit: {exc}", file=sys.stderr)
        return 2

    abtop_status = read_abtop_status() if args.with_abtop else None
    status = summary_to_json(windows, abtop_status)

    if args.json:
        print(json.dumps(status, ensure_ascii=False, indent=2))
    else:
        print_human(windows, abtop_status)

    return exit_code(status, args.fail_on)


def print_human(windows, abtop_status) -> None:
    print("NeuroGate limits")
    if not windows:
        print("  usage rows not found in /v1/me response")
    for window in windows:
        reset = format_duration(window.reset_in_seconds)
        print(f"  {window.label:<4} {window.level:<7} reset {reset}")
        if window.credits:
            print(f"       credits  {format_metric(window.credits)}")
        if window.requests:
            print(f"       requests {format_metric(window.requests)}")

    if abtop_status:
        print("\nLocal agents from abtop")
        for agent in abtop_status.get("agents", []):
            print("  " + format_agent(agent))


def format_metric(metric) -> str:
    return f"{metric.used:g}/{metric.limit:g} ({metric.percent:.1f}%, left {metric.remaining:g})"


def format_duration(seconds: int | None) -> str:
    if seconds is None:
        return "unknown"
    if seconds < 60:
        return f"in {seconds}s"
    if seconds < 3600:
        return f"in {seconds // 60}m"
    if seconds < 86400:
        return f"in {seconds // 3600}h {(seconds % 3600) // 60}m"
    return f"in {seconds // 86400}d {(seconds % 86400) // 3600}h"


def format_agent(agent: dict) -> str:
    agent_cli = str(agent.get("agent_cli") or "agent")
    sessions = agent.get("sessions", "?")
    active = agent.get("active", "?")
    tokens = agent.get("active_tokens", "?")
    context_pct = agent.get("max_context_pct")
    try:
        context = f"{float(context_pct):.0f}%"
    except (TypeError, ValueError):
        context = "n/a"
    return f"{agent_cli:<8} sessions {sessions:<2} active {active:<2} ctx max {context} tokens {tokens}"


def exit_code(status: dict, fail_on: str) -> int:
    if fail_on == "never":
        return 0
    levels = [window.get("level") for window in status.get("windows", [])]
    if fail_on == "danger" and "danger" in levels:
        return 3
    if fail_on == "warning" and any(level in ("warning", "danger") for level in levels):
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
