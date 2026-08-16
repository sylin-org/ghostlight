# In-page browser affordance (deferred)

Date: 2026-08-16

Status: deferred. Authored on 2026-08-15 as stage S5 of the reference-experience epic, moved here
unchanged on 2026-08-16 when the epic was reworked. It is not scheduled and not cancelled.

Two facts stand behind the deferral. The tree has no authentic full-vector mascot: every mascot
asset is raster and the only vector mark is `extension/icons/ghost-mark.svg`, so the stage's own
stop condition held from the start. And of the five audiences named in
[the epic bootstrap](../tasks/reference-experience/BOOTSTRAP.md), none both wants presence on its
pages and is likely to be on Linux, which is where the epic's evaluation runs.

The reasoning below is preserved as written. If the affordance returns, it needs a vector source
with recorded provenance and a named audience first.

## Objective

Give every controlled page a small, recognizable, optional doorway to At a glance without placing
runtime controls or product policy in page content.

## Prompt outline

1. Resolve the identity asset before implementation. Preserve the 100x100 pixel mascot at its
   native artwork grid. Use an authentic full vector source for arbitrary small sizing; do not
   auto-trace or blur the pixel artwork. The existing simplified vector ghost mark may serve the
   minimal pill.
2. Implement the accepted closed presence choices, expected to distinguish a branded mascot,
   inconspicuous minimal mark, and hidden presentation. Keep one owner for the preference.
3. Show the affordance only in the states accepted by S1. It must remain content-free, avoid recent
   focus and interaction, and intercept pointer input only on its exact accessible target.
4. Clicking the affordance requests the existing authenticated native activation path and opens or
   focuses the workbench on At a glance. It must not navigate the controlled page.
5. Keep pause, resume, stop, panic, policy, and diagnostic behavior in the GUI and existing fallback
   surfaces. The affordance is a doorway, not another control panel.
6. Preserve successful browser work when injection, rendering, activation, or the workbench view
   fails. Test page navigation, document replacement, zoom, scaling, reduced motion, and hostile
   page CSS.

## Completion evidence

- Approved exact-size or vector assets with recorded provenance.
- Branded, minimal, and hidden modes behave as one closed setting.
- Click reaches At a glance through the existing native activation authority.
- Presentation failure cannot change authority, execution, or outcomes.
- Accessibility and collision behavior pass focused extension and live visual checks.
- No page-level duplicate of workbench controls exists.

## Stop conditions

- The only available mascot would require arbitrary pixel-art resizing or automatic tracing.
- Page code must decide product state, authority, or control semantics.
- Opening the workbench requires a second service or activation route.
- A hostile page can intercept the activation or the affordance blocks ordinary page use.
