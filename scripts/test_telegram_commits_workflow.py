#!/usr/bin/env python3
"""Contrato estático del workflow de avisos Telegram por commit."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "telegram-commits.yml"


class TelegramCommitsWorkflowTest(unittest.TestCase):
    def test_sends_a_torusocean_style_push_summary_without_exposing_secrets(
        self,
    ) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        for marker in (
            "push:",
            'branches:\n      - "**"',
            "TELEGRAM_BOT_TOKEN: ${{ secrets.TELEGRAM_BOT_TOKEN }}",
            "TELEGRAM_CHAT_ID: ${{ secrets.TELEGRAM_CHAT_ID }}",
            "PUSH_COMPARE: ${{ github.event.compare }}",
            "PUSH_PROJECT_LABEL: OpenTTDRS",
            "(.commits // []) | length",
            ".head_commit.author.username // .pusher.name",
            "🚀 %s · push recibido",
            "Repositorio: %s",
            "Ref: %s",
            "Commit: %s",
            "Autor: %s",
            "Commits recibidos: %s",
            "Mensaje: %s",
            'short_sha="${PUSH_AFTER:0:12}"',
            'compare_url="$PUSH_COMPARE"',
            "https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}/sendMessage",
            '"chat_id=$TELEGRAM_CHAT_ID"',
            "--data-urlencode \"text=$text\"",
            "set -euo pipefail",
            "env.PUSH_DELETED != 'true'",
        ):
            self.assertIn(marker, workflow)
        self.assertNotIn("git rev-list --reverse", workflow)
        self.assertNotIn("actions/checkout", workflow)
        self.assertNotIn("echo $TELEGRAM_BOT_TOKEN", workflow)
        self.assertNotIn("echo $TELEGRAM_CHAT_ID", workflow)


if __name__ == "__main__":
    unittest.main()
