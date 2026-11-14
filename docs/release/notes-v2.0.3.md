# Ghostlight 2.0.3

Ghostlight 2.0.3 stops a tab from refusing all work after it leaves a page with nested embedded
frames, such as Office for the web in SharePoint, and makes screenshots of such pages come out
whole. Update with the same command you installed with:
`irm https://sylin.org/ghostlight/get.ps1 | iex` on Windows,
`curl -fsSL https://sylin.org/ghostlight/get.sh | sh` on Linux, or `npx -y ghostlight install`.

Two of these fixes are in Ghostlight in Browser 2.0.2, which arrives through the Chrome Web Store
once it is approved and published. The rest work with the extension you already have.

### Fixed

- **A tab no longer gets stuck refusing everything.** After a tab left a page that holds a frame
  from one site inside a frame from another, every read, click, scroll and key press on that tab
  was refused with "Document access could not be verified", on any page it showed afterwards,
  until the browser restarted. With no policy that restricts documents, this release alone fixes
  it. The cause is removed in Ghostlight in Browser 2.0.2, which is also what fixes it under a
  restricting policy (ADR-0207).
- **A point below the fold works.** A click or scroll at a point of a full-page screenshot that
  is not on screen was refused. The point is now scrolled into view and used.
- **A wait that runs out of time says so.** A wait for text that never appeared could report
  that Ghostlight and the extension did not agree and ask for an extension update. It now says
  the text never appeared.
- **Screenshots of embedded apps are whole** (Ghostlight in Browser 2.0.2). A capture of what is
  on screen no longer touches the page. Before, a busy embedded app could come back blank or
  squeezed into a corner.
- **Changing browsers does not stop the work.** When the browser a session was using goes away
  and exactly one other browser is connected, the session carries on there. With several
  connected it still asks which one.

### Changed

- **A refusal about a document says why.** `document_unavailable` keeps its code and adds a
  `document_cause` fact, with a sentence for that cause and next steps that can work.
- **A check that text is gone says when it could not tell.** When part of the page could not be
  read, the answer says absence could not be established. It is still not counted as gone.
- **A magnified region is an enlargement of what is on screen** (Ghostlight in Browser 2.0.2). It
  no longer re-renders the page for extra detail.

Policy, Pause and Stop, and what a restricting policy refuses are unchanged.
