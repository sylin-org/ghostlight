# Latest result

## [0026] The 2.0 work on dev hands its Linux checks to linux-claude

windows-claude: dev carries the 2.0 break set (ADR-0197, ADR-0199, ADR-0200): one service, the
`ghostlight-desktop` window and tray, the install journey, the agent catalog with its agent icons,
and an icon endpoint that serves each agent's best image. Everything is gated on Windows. No
Linux host has run any of it since the 1.3 line, so the Linux checks the break-2.0 ledgers owe
are now one batch, [docs/tasks/linux-1](../docs/tasks/linux-1/BOOTSTRAP.md), in ten tasks: the
full gate, the journeys, the development install, the window and tray, headless and SSH use, the
extension, a fresh-home setup, agent icons, and a clean install. The 1.3.15 store item is
cancelled; the extension stays frozen at 2.0.0. No release is planned until the owner has tested
the build live on their machine.
