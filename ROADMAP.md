# Roadmap

Ghostlight ships today as a governed browser-automation MCP server for Chromium, verified
end-to-end on Windows. This page is what we are working toward next. Nothing here changes the
Continuity Promise or the trained tool surface.

## Near term

- **Chrome Web Store listing.** Install the extension without developer mode.
- **One continuous install journey.** Whichever half a user finds first -- extension or local
  service -- leads directly to the other, with idempotent client registration and one clear restart
  step.
- **Live browser verification on macOS and Linux.** Both already build and pass the full test
  suite in CI; this brings end-to-end browser coverage on par with Windows.
- **Finish the lightbox migration.** Move the remaining process-spawn scenarios into the faster,
  more inspectable browser and governance harness without reducing coverage.
- **Confirm irreversible actions.** Add the designed out-of-band confirmation boundary for send,
  delete, purchase, publish, and similar consequences.

## Direction

More adapters will follow on the same governance spine. The browser is the first surface, not
the last. The durable asset is the [RAWX capability model](open-spec/rawx-capability-model.md);
the mechanisms around it will change.

Two proposed directions are being explored rather than promised: local evaluation artifacts for
comparing agent journeys ([ADR-0069](docs/adr/0069-agent-journey-evaluation-artifacts.md)), and
bounded delegation contracts grounded in a concrete user scenario
([design note](docs/design/bounded-delegation-scenario.md)). WebMCP remains a research and standards
participation track until its browser API stabilizes.

Have a request? [GitHub Discussions](../../discussions) is the place, and every request gets a
disposition with reasoning.
