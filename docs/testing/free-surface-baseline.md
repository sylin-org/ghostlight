# Free-surface baseline recipe

This recipe measures the current Ghostlight shape before either research-18 candidate exists. It
uses deterministic fixture content only. It does not enable annotated screenshots or tab labels,
and it does not change the public MCP schema.

## Automated mechanical baseline

Run on the same Linux or macOS environment used by the real-browser smoke gate:

```sh
node tests/e2e/run-smoke.mjs --free-surface-baseline
```

To inspect the resolved paths without launching Chromium:

```sh
node tests/e2e/run-smoke.mjs --dry-run --free-surface-baseline
```

The runner starts the real Ghostlight MCP edge, persistent service, browser relay, unpacked
extension, Chromium, and a local fixture server. Its JSON report contains:

- Ghostlight version, platform, and measurement time;
- three visual-semantic journeys;
- current observation calls, text characters, base64 image characters, and elapsed time;
- current opaque Ghostlight tab handles and their character count; and
- explicit limits on what the measurement proves.

The blocking Linux `e2e-smoke` CI job runs this mode after the ordinary browser smoke. A passing
job proves the mechanical baseline still works across the real process and extension boundary. It
does not replace the visible-browser or repeated model-behavior gates below.

Candidate A's current measurement is `browser_take_screenshot` plus `browser_read_page`: two
complementary observations. Candidate B records opaque handles from `browser_list_tabs`. The
report is a lower-level payload baseline. It does not simulate model judgment.

The first manual Codex/Windows result is recorded in research 18. It validates the fixture and the
current two-observation shape. It is not a substitute for this automated run.

Keep raw reports local unless their fixture-only contents have been inspected. If a report is
committed later, store a normalized version that removes measurement time, process-specific ids,
ports, temporary paths, and image bytes.

## Model-behavior runs

Acceptance still requires repeated model runs. Use the same client, model, viewport, prompt, and
Ghostlight version for baseline and candidate configurations. Start each run in a new conversation
and a new Ghostlight tab.

Serve the fixture locally. One portable option is:

```sh
python -m http.server 8765 --directory tests/e2e
```

Run these prompts three times per configuration:

1. Dense toolbar

   ```text
   Open http://127.0.0.1:8765/free-surface-fixture.html?journey=toolbar in a new Ghostlight tab.
   Identify the control named Review changes and report the exact ref you would use. Do not click.
   Do not use JavaScript.
   ```

2. Repeated form

   ```text
   Open http://127.0.0.1:8765/free-surface-fixture.html?journey=form in a new Ghostlight tab.
   Identify the approved invoice amount field and report the exact ref you would use. Do not edit.
   Do not use JavaScript.
   ```

3. Mixed viewport

   ```text
   Open http://127.0.0.1:8765/free-surface-fixture.html?journey=viewport in a new Ghostlight tab.
   Identify the Review control visible before scrolling and report the exact ref you would use.
   Do not click or scroll. Do not use JavaScript.
   ```

4. Multi-tab recovery

   ```text
   Open the alpha, beta, and gamma product variants of the free-surface fixture in three new
   Ghostlight tabs. Compare their prices, navigate each tab once to its same URL, then return to
   the cheapest product and report the tab you chose. Do not click Select.
   ```

Product URLs use `journey=product&product=alpha`, `beta`, or `gamma`.

For each run record:

| Field | Meaning |
|---|---|
| configuration | client, model, Ghostlight commit, and candidate state |
| journey | one of the four prompts above |
| tool_calls | all Ghostlight calls after the initial prompt |
| observation_calls | screenshot, read, find, or context calls |
| model_visible_text_chars | serialized text returned to the model |
| screenshots | number of image results |
| recovery_turns | calls made only because the first observation was insufficient or stale |
| wrong_target | whether the model selected the wrong ref or tab |
| elapsed_ms | prompt to terminal response |
| terminal_result | exact selected ref or tab and whether it is correct |

Do not infer token counts from text characters. Use a client-provided token measure when one is
available and identify its tokenizer. Otherwise report characters as a stable proxy.

## Decision boundary

Apply the thresholds in [research 18](../research/18-free-surface-evaluation-plan-2026-07.md).
Deterministic payload arithmetic can justify building a prototype. Only repeated model runs can
justify a claim about fewer recovery turns or better target selection.
