# Tool visual-signature coverage

This is the coverage ledger for Ghostlight's complete tool surface. The normative shapes, colors,
motion, and safety invariants live in [visual-language.md](visual-language.md). ADR-0083 defines the
classification and shared signature-medallion architecture.

The question is not "can this tool have an animation?" It is "what does a person need to know while
this action touches their browser?" Quiet is an intentional answer.

## Presentation classes

| Class | Use when |
| --- | --- |
| Content | The user needs text, destination, explanation, or a decision |
| Spatial | The action has a meaningful point, path, field, or region |
| Signature | Non-spatial activity is worth disclosing without arbitrary text |
| Native | Browser or page behavior already makes the result clear |
| Quiet | The operation does not touch the page, or another layer already explains every substep |

## Coverage ledger

| Tool or action | Class | Current signature | Decision or open item |
| --- | --- | --- | --- |
| `tabs_context_mcp` | Quiet | none | Reads Ghostlight topology, not the page |
| `tabs_create_mcp` | Native | new tab plus controlled-tab border | Sufficient scope disclosure |
| `navigate` | Content | destination pill after landing | Keep host/path pill |
| `computer.left_click` | Spatial | cursor, target glow, click ripple | Keep |
| `computer.right_click` | Spatial | cursor, target glow, dashed ripple | Keep |
| `computer.double_click` / `triple_click` | Spatial | cursor, target glow, rhythmic ripples | Keep |
| `computer.hover` | Spatial | cursor glide | Keep |
| `computer.left_click_drag` | Spatial | cursor and comet trail | Keep |
| `computer.type` | Spatial + signature | field shimmer plus keyboard medallion | Never render the typed value |
| `computer.key` | Content | named key-chord lozenge | Keep; review printable-key masking separately |
| `computer.scroll` | Spatial | cursor and directional chevrons | Keep |
| `computer.scroll_to` | Native | visible page movement | Candidate: chevrons settling into a target halo |
| `computer.screenshot` | Spatial + signature | capture frame plus camera medallion after capture | Shared confirmation signature |
| `computer.zoom` | Spatial | converging region frame | Keep |
| `computer.wait` | Signature | three fading lights for the actual wait lifetime | Replaces the fixed-duration pulse |
| `find` | Spatial | page scan | Keep; finding is inspection, not mutation |
| `form_input` | Spatial | field-shaped splash | Keep |
| `get_page_text` | Spatial | page scan | Keep |
| `javascript_tool` | Signature | workwheel for the actual evaluation lifetime | Fixed, content-free signature |
| `read_console_messages` | Quiet | none | Candidate: shared backstage diagnostic signature |
| `read_network_requests` | Quiet | none | Candidate: shared backstage diagnostic signature |
| `read_page` | Spatial | page scan | Keep |
| `resize_window` | Native | window visibly resizes | No additional signage |
| `update_plan` | Quiet | none | Local MCP information only |
| `narrate` | Content | narration caption | The tool is its own presentation |
| `wait_for` | Signature | three fading lights for the actual condition wait | Replaces the post-completion pulse |
| `script` | Quiet composite | visible substep signatures | Never add an umbrella effect |
| `form_fill` | Spatial composite | sequential field splashes and submit click | Keep one treatment per touched control |
| `act_on.left_click` / `right_click` / `double_click` | Spatial composite | semantic target cue plus click treatment | Keep |
| `act_on.hover` / `scroll_to` | Spatial composite | semantic target cue plus underlying movement | Keep |
| `act_on.set_value` | Spatial composite | semantic target cue plus field splash | Keep |
| `dialog.status` | Native | browser dialog is already visible | No additional signage |
| `dialog.accept` / `dismiss` / `respond` | Native | native dialog disappears or responds | No additional signage |
| `tab_control.focus` / `reload` / `close` | Native | browser focus, reload, or tab removal | No additional signage |
| `file_upload` | Spatial | field splash on the file control | Keep; never display filenames in extension chrome |
| `browser_batch` | Quiet composite | visible substep signatures | Never add an umbrella effect |
| `upload_image` by ref | Spatial | field splash | Keep |
| `upload_image` by coordinate | Native | page receives a drop | Candidate: photo tile entering a target halo |
| `gif_creator.start_recording` / `stop_recording` | Native persistent | truthful Chrome REC badge and popup state | Never replace with an in-page simulated REC state |
| `gif_creator.status` / `clear` | Quiet | none | No live page operation |
| `gif_creator.export` | Native or spatial | browser download UI or upload target | Reuse a future image-drop cue; no umbrella badge |
| `explain` | Quiet | none | Local MCP directory only |

## Review queue

These remain proposals, not promised effects:

1. Mask printable `computer.key` input while retaining named shortcuts and navigation keys.
2. Give direct `scroll_to` a destination treatment without duplicating visible page motion.
3. Give coordinate image placement a recognizable photo-drop treatment.
4. Decide whether console and network inspection need one shared backstage diagnostic signature or
   are better left quiet for ordinary users.

Any accepted item must update this ledger and the normative visual vocabulary with its code.
