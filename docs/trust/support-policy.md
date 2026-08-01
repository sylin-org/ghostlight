# Ghostlight Support Policy

This page states the support channel, what we commit to, and what support covers.

## Channel

Support runs by email at support@sylin.org. It is read by the maintainer. Product questions,
installation and configuration help, policy authoring, and central-policy deployment all
belong there. Suspected security vulnerabilities do not: those go through the private channel
documented in [SECURITY.md](../../SECURITY.md), never to the support lane, so that a report is
handled under disclosure rules, not as a support ticket. The current private reporting address and
required subject line are stated in SECURITY.md.

## Response commitment

The commitment is a first human acknowledgment, not a resolution. We acknowledge Team support
requests within 3 business days and Enterprise support requests within 2 business days. The
clock measures the time to a first human reply that confirms we have the request and are
looking at it; it does not promise that the issue is resolved within that window, because
resolution time depends on the issue. Business days are Monday through Friday, measured in UTC.
In practice acknowledgment is typically much faster, but the commitment above is what we hold
ourselves to.

## Scope

Support covers the things you need to run Ghostlight: installation, configuration, authoring
and troubleshooting policy, and `managed://` central-policy deployment. It does not cover
custom development, nor the operation of your own MCP client or the model behind it, which are
outside Ghostlight and belong to you and your provider. Where an issue turns out to be a
defect in Ghostlight, it is handled through the normal release process: fixes land on the
latest tagged release, and pre-1.0 there are no backport branches (see
[SECURITY.md](../../SECURITY.md)).

## Enterprise extras

Enterprise includes deployment help and roadmap input in addition to the faster
acknowledgment window: hands-on assistance standing up central policy and audit in your
environment, and a standing channel for roadmap input. Enterprise also covers completed
security questionnaires: where the published CAIQ-shaped self-assessment does not satisfy a
portal, we complete one full questionnaire per year on request. These match the Enterprise
commitments on the pricing page.

Last reviewed: 2026-07-14 against v0.7.2 | Contact: support@sylin.org
