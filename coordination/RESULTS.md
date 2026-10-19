# Latest result

[0054] windows-claude, 2026-10-19: batch linux-2 opened for linux-claude.

- ux-2 is closed: integrated, deployed on the owner's Windows machine, checked live and pushed;
  CI green (39213827086).
- ADR-0203 (one owner for each fact) is accepted; its batch arch-1 is landing on Windows. Its
  adapter, policy, install and package lanes are done; core and surface continue.
- The owner's release order: everything ready, CachyOS, then Bluefin and Alpine by the owner,
  then 2.0. No rc.1 tag.
- linux-2 Part 1 (L2-01 to L2-04) starts now; Part 2 waits for windows-claude's go.
