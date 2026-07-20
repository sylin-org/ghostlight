# Open-source publication paths, 2026-07

**Status:** research complete; no external publication authorized

**Date:** 2026-07-18

**Scope:** how open-source developer tools are first discovered, amplified, and converted into
real use; implications for a future Ghostlight public-awareness effort

## Executive conclusion

Trending feeds do not make a repository trend. They report a result that already happened.

The Bluesky account that prompted this study is an unofficial bot. Its source says it crawls
GitHub Trending, posts a trending repository every hour, and suppresses recently posted projects.
In a 100-post snapshot from July 11 through July 18, 2026, 53 posts linked to 50 unique
repositories. The median post had zero likes and zero reposts. The repositories in those posts
were already reporting daily gains of hundreds or thousands of stars. The bot is a free, late
amplifier with little direct engagement, not an entrance to the discovery system.

Across the projects studied, durable growth followed the same broad path:

```text
useful problem + legible proof + easy first use
                  |
                  v
      one or more high-affinity communities
                  |
                  v
     stars, installs, discussion, and sharing
                  |
                  v
 GitHub Trending, curators, directories, and bots
                  |
                  v
       a larger second discovery wave
```

There was no universal launch channel. Some projects grew without Hacker News. Some grew through
an existing maintainer audience. Some saturated ecosystem directories. One attached a memorable
number to a technically credible result. One occupied an already active category-level grievance.
The common denominator was not a post location. It was a message that a relevant person could
understand, try, verify, and repeat.

The best Ghostlight strategy is therefore a staged publication, not a simultaneous link dump:

1. make the first successful use easy enough to survive broad traffic;
2. seed the project among MCP-client and local-agent practitioners who have the exact problem;
3. publish one founder-present anchor launch with a runnable artifact and visible proof;
4. adapt the same truth to each community instead of syndicating identical copy;
5. earn later directory, curator, newsletter, and trending-bot amplification;
6. judge success by meaningful visits, installs, successful first-use reports, support demand,
   repeat users, and contributors -- not stars alone.

## Research question and method

The investigation asked four separate questions:

1. What does a trending-project bot actually select?
2. What happened before representative repositories appeared in that feed?
3. Which paths produced awareness, and which left evidence of adoption?
4. Which current channels fit Ghostlight's practitioner-first, local, visible, governed product?

The sample intentionally mixed adjacent agent tooling with projects that illustrate a distinct
publication mechanism:

- `Nutlope/hallmark`: visual proof and an established developer audience;
- `Dicklesworthstone/destructive_command_guard`: recurring fear, community presence, and an
  adjacent tool ecosystem;
- `injaneity/pi-computer-use`: host-ecosystem fit, package distribution, and curator pickup;
- `KnockOutEZ/wigolo`: MCP directory saturation and machine-readable distribution metadata;
- `RyanCodrai/turbovec`: a memorable numerical result that media and social accounts could repeat;
- `OpenCut-app/OpenCut`: a strong alternative-to category and a caution about vanity metrics.

Evidence came from repository history and metadata, package registries, GitHub releases and
traffic, Hacker News search, public social posts, community discussions, directory listings, and
the bot's public AT Protocol feed. Counts are point-in-time observations, not permanent facts.
Package downloads can include CI, bots, mirrors, updates, and repeat installs. GitHub release
downloads can include multiple operating-system assets per person. They are useful directional
signals, not user counts.

## The bot is downstream

[`github-trending.bsky.social`](https://bsky.app/profile/github-trending.bsky.social) describes
itself as an unofficial GitHub Trending auto-post bot. Its
[public source](https://github.com/kawamataryo/bsky-github-trending-bot) documents the selection
path: crawl GitHub Trending, post repositories on a schedule, and add summaries.

Snapshot taken at approximately 2026-07-18 19:45 UTC through the public
[`app.bsky.feed.getAuthorFeed` endpoint](https://docs.bsky.app/docs/api/app-bsky-feed-get-author-feed):

| Observation | Result |
|---|---:|
| Feed items inspected | 100 |
| Repository-link posts | 53 |
| Unique repositories | 50 |
| Median likes | 0 |
| Maximum likes | 5 |
| Median reposts | 0 |
| Maximum reposts | 1 |

The causal order matters. GitHub says stars influence repository rankings and Explore results, and
the bot reads a ranking produced from that upstream activity. There is no submission form for the
bot and no legitimate technique for making it choose a repository directly.

Treat this class of bot as:

- evidence that a project has entered a broader discovery wave;
- a possible extra backlink and social mention;
- a way to observe which messages are being compressed into one sentence;
- never a campaign target or a success metric.

## Case findings

### Hallmark: visible result plus an existing audience

Hallmark opens with an unusually legible promise: a design skill that refuses to look
AI-generated. Its repository supplies a visual gallery, four named verbs, a live demo, a
one-command install, and support for several coding agents. The
[creator launch post](https://www.linkedin.com/posts/nutlope_announcing-hallmark-an-open-source-design-activity-7462523734868795392-oWjC)
used the same compact structure: what it changes, who it works with, four things to try, one install
command, and a request for feedback.

No material Hacker News path was discoverable. The stronger explanation is the combination of an
established developer creator, a Together AI association, a striking before-and-after category,
and an artifact that people could understand from a short video without first reading an
architecture document.

**Lesson:** visible transformation is distribution infrastructure. A memorable point of view and
a ten-second proof can make the user retell the project accurately.

### Destructive Command Guard: live where the fear occurs

[`destructive_command_guard`](https://github.com/Dicklesworthstone/destructive_command_guard)
solves a recurring, emotionally legible failure: an agent destroying work. Its creator's
[launch explanation](https://www.linkedin.com/posts/jeffreyemanuel_agent-coding-life-hack-im-100-convinced-activity-7421442482082660352-l5AG)
names the fear, explains why naive matching is inadequate, gives a one-line installer, and shows
how denials help the agent recover rather than merely stopping it.

Three Hacker News submissions received only two or three points. That channel did not create the
project's growth. The stronger path was an established maintainer audience, an integrated family
of adjacent agent tools, frequent releases, and organic recommendations inside the communities
where destructive-agent behavior is discussed. As of the research snapshot, 31 releases carried
423 assets with about 56,000 aggregate downloads; the latest release alone had more than 6,100.

**Lesson:** reputation and repeated usefulness compound. Being recommended in the conversation
about the problem is more valuable than broadcasting to a large but indifferent audience.

### pi-computer-use: fit tightly into an active host ecosystem

[`pi-computer-use`](https://github.com/injaneity/pi-computer-use) gives Pi users an accessibility-
first computer-control surface with a one-command package install. Its README plainly separates
what the package does from what it does not do and supplies enough implementation detail for
technical curators to form a story.

It had no meaningful Hacker News path. Discovery instead appeared through the Pi ecosystem,
package search, star-feed and curator accounts, and later trending coverage. The npm package had
2,041 downloads from June 18 through July 17, 2026. Third-party summaries sometimes overstated or
altered the project's claims, which is itself a distribution finding.

**Lesson:** close host integration creates a ready-made audience. It also creates narrative risk:
once curators take over, a canonical short explanation and explicit non-claims become important.

### Wigolo: make every discovery index able to understand the project

[`wigolo`](https://github.com/KnockOutEZ/wigolo) combines one-command multi-client setup with
diagnostics, uninstall support, a demo, comparisons, benchmarks, `llms.txt`, MCP metadata, and
directory-specific manifests. Search found it across MCP directories including Glama, MCP.so,
MCPServers.org, Cursor-oriented listings, and other mirrors. It had 1,959 npm downloads during the
same June 18 through July 17 window.

There was no prominent Hacker News or founder-audience launch in the evidence. Directory and
registry saturation is the more plausible seed path. Some directory pages were already stale or
wrong: tool detection failed on one, an obsolete namespace appeared on another, and star counts
lagged.

**Lesson:** machine-readable metadata and ecosystem directories can create many discovery edges,
especially for MCP software. Every extra mirror can also fork the truth. The canonical install and
capability source must remain obvious, and listings require periodic review.

### TurboVec: a number that travels

[`turbovec`](https://github.com/RyanCodrai/turbovec) has a highly repeatable proof: a stated
31 GB float32 vector corpus can fit in about 4 GB under its documented conditions, with a speed
comparison against FAISS. The repository pairs the number with benchmarks, technical lineage,
Python and Rust surfaces, and `pip install turbovec`.

Its exact Hacker News submission received four points and no comments. The numerical hook traveled
further through technical media and then social posts. By the snapshot, PyPI reported 40,194
downloads in the previous month and 14,615 in the previous week. Some amplifiers changed "vector
index memory" into "AI model memory" and misattributed the implementation to Google; a
[public correction thread](https://www.linkedin.com/posts/aeejazkhan_sam-altman-has-a-new-problem-google-just-activity-7469255919818792960-uxyW)
shows both the power and danger of a portable claim.

**Lesson:** one honest, bounded number is a distribution primitive. Publish the conditions and
tradeoffs beside it, because earned media will tend to remove both.

### OpenCut: category pain travels, but suspicious metrics subtract trust

OpenCut's phrase, "the open-source CapCut alternative," maps immediately onto a large existing
audience and active frustration about subscriptions, paywalls, platform support, and terms. A
[2025 Hacker News submission](https://news.ycombinator.com/item?id=44553752) reached 447 points and
151 comments. In Reddit discussions about CapCut alternatives, users now recommend OpenCut without
the maintainer introducing it. That is genuine category-level word of mouth.

The project also had more than 75,000 stars and 7,500 forks by the snapshot while users in recent
threads openly questioned whether those counts were organic. This study does not establish that
the project bought or manufactured stars. It does establish that anomalous vanity metrics can
reverse their intended effect: they make prospective users investigate the metric instead of the
software.

**Lesson:** an "open-source alternative to X" message is powerful when a community already feels
the pain. Do not optimize for star velocity. Trust, product maturity, and successful use have to
support the attention.

## The publication path that generalizes

### 1. Conversion readiness comes before reach

A high-attention post exposes every onboarding defect at once. Before seeking it, the project needs:

- a one-sentence problem and differentiated answer;
- a canonical home URL;
- a repository description, homepage, topics, and social preview;
- a proof artifact visible without installing;
- the shortest honest install path;
- one first task that reaches a meaningful result quickly;
- supported platforms and non-goals stated before the user wastes time;
- licensing, security, privacy, contribution, and support facts within easy reach;
- enough release and issue hygiene to show that trying the project is not abandonment risk.

This is not cosmetic preparation. It determines whether awareness becomes use.

### 2. Start with the smallest audience that has the exact problem

The best seed communities are normally host ecosystems, adjacent tools, package registries,
problem-specific discussions, and people already asking for a solution. Their feedback repairs
the conversion path before a general audience arrives. Their authentic use also creates the
recommendations and links that broader curators can discover.

### 3. Give the launch one anchor artifact

An anchor is the canonical, discussable event: a Show HN, a technical article, a demo launch post,
or a substantial release announcement. It needs a founder present to answer questions and one
primary link where the visitor can understand and try the software.

The anchor is not copied everywhere. It is the source from which audience-specific explanations
are derived.

### 4. Adapt, do not syndicate

| Audience | Lead with | Proof | Ask |
|---|---|---|---|
| MCP and coding-agent practitioners | The workflow they cannot complete cleanly today | Install plus a real task | Try it in a named client; report friction |
| General developers | The architectural distinction and why it matters | Short demo plus source | Challenge the design; try the project |
| Local-first and privacy users | Local execution, ordinary user context, and no account | Architecture and network boundary | Verify the claims |
| Security and governance practitioners | Capability policy, identity, audit, and explicit limits | Trust documents and decision aid | Review threat and control boundaries |
| Non-developer MCP users | Visible control and immediate useful outcome | Calm visual story | Try a documentation-heavy task |

### 5. Let secondary systems amplify evidence

GitHub Trending, Bluesky bots, directory mirrors, star feeds, newsletters, and roundup writers are
secondary systems. They become more likely to notice when primary communities produce links,
discussion, installs, and stars in a short period. They should be monitored and corrected, not
gamed.

### 6. Convert attention into a maintained relationship

Answer questions while the launch is active. Turn repeated confusion into documentation. Ship
small fixes quickly. Thank users who provide evidence. Make contribution paths bounded. Publish
substantive progress later without pretending every version is a new launch.

## Channel map

| Channel | Role | How to use it well | Main constraint |
|---|---|---|---|
| GitHub repository and Releases | Canonical proof and conversion | Lead with outcome, visual proof, install, first task, truth, and active releases | Stars are bookmarks and ranking signals, not users |
| Package and official ecosystem registries | Intent-rich discovery and install | Complete metadata; keep versions and canonical links current | Downloads include automation and repeats |
| MCP client communities | Highest-fit seed users | Demonstrate the exact client workflow; ask for setup and tool-quality feedback | Participate according to each project's norms; do not hijack support threads |
| Hacker News / Show HN | Founder-present technical anchor | Link a runnable project, explain why and how, stay available, accept direct criticism | No signup barrier, landing-page-only post, vote solicitation, or trivial update |
| Lobsters | Deep technical discussion | Submit a durable architecture or implementation artifact if already participating | Invite-only; self-promotion should be under one quarter of activity |
| Reddit | Problem-specific conversation | Read current rules, disclose authorship, write a useful native post, answer every serious question | Many communities enforce account history, karma, and roughly 10:1 non-promotional participation |
| Bluesky | Developer social graph, search, and custom feeds | Use the visual proof, concise text, accurate keywords/hashtags, alt text, and replies | There is no single universal algorithm; relevant relationships and feed rules matter |
| LinkedIn | Founder network and organizational context | Tell the practical story in plain language; put governance implications after the product proof | Generic launch copy reads as corporate promotion |
| YouTube or a durable demo video | Searchable proof and onboarding | Show installation, first use, and the visible browser experience with chaptered links | Version-specific setup videos decay and must name the tested release |
| DEV or a project blog | Searchable technical depth | Publish one useful implementation or design article that stands alone | An article that only points at the repo creates little value |
| Product Hunt | Later broad-product discovery | Use when install is polished and the high-craft visual experience is ready for immediate use | Audience fit is weaker than practitioner channels; not every submission is featured |
| Newsletters and curators | Earned second wave | Offer a short factual brief, proof link, author availability, and non-claims | They may simplify or distort the story; never buy editorial disguise |
| General MCP directories | Long-tail ecosystem search | Prefer official registry first; add selected directories only when local stdio packaging is represented honestly | Listings become stale and some favor hosted/remote products that do not match Ghostlight |

Two current platform details are especially important:

- [Show HN](https://news.ycombinator.com/showhn.html) is explicitly for something the audience can
  try. It forbids asking friends to upvote or comment.
- [Lobsters](https://lobste.rs/about) says author participation is welcome but write-only
  self-promotion is not; less than one quarter promotional activity is its rule of thumb.
- Reddit rules must be checked again immediately before every submission. As of this study,
  r/LocalLLaMA had recently added minimum community karma and stricter self-promotion enforcement.
- Bluesky uses following timelines, Discover, search, and user-selected custom feeds. Keywords and
  hashtags can enter topic-specific discovery paths, but relationships and replies still matter.
- Product Hunt now asks whether a product is useful, novel, high craft, or creative and requires a
  live product for featuring. That makes it a possible later visual-product channel, not the first
  Ghostlight seed channel.
- GitHub Release Radar is stale launch advice. Its
  [submission repository](https://github.com/github/release-radar) was archived in March 2025 and
  should not appear in a current channel plan.

## Message anatomy

The strongest observed messages contained six compact elements:

1. **Problem:** a failure the reader already recognizes.
2. **Difference:** one sentence that separates the project from familiar alternatives.
3. **Proof:** a visual transformation, measured result, or live workflow.
4. **First use:** one command or one short install journey.
5. **Boundary:** what it does not do, especially where a category invites a wrong assumption.
6. **Specific invitation:** a named task to try or a focused question to answer.

For an open-source launch, "free" is not enough. State the license boundary accurately, but lead
with the user's outcome. Readers who have been burned by future paywalls need concrete facts: what
is licensed how, what runs without an account, what is separately licensed, and whether activation
or telemetry exists.

## Awareness is not adoption

Use a layered scorecard:

| Layer | Useful signals | Do not over-interpret |
|---|---|---|
| Reach | Unique repository visitors, referring sites, post views | Raw impressions |
| Interest | README depth, release-page visits, stars, saves, substantive questions | Star total alone |
| Trial | Package downloads, release-asset downloads, install questions, first-run reports | Download count as unique people |
| Use | Repeated issue context, users sharing real tasks, dependents, repeat downloads across releases | Anonymous clone spikes |
| Community | Repeat contributors, outside documentation, organic recommendations, integrations | Drive-by comments |
| Trust | Accurate third-party descriptions, security questions answered, claims verified | Uncritical praise |

[GitHub Traffic](https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-traffic-to-a-repository)
retains only 14 days of visitors, clones, referrers, and popular content. Archive a snapshot before
and after each publication event. GitHub states that stars help people bookmark and discover
related projects and influence rankings; that makes them a useful awareness signal, not proof of
use.

Ghostlight must not add telemetry or phone-home behavior to improve campaign measurement. Public
repository traffic, registry counters, release counters, community responses, and voluntary user
reports are sufficient. Canonical, untagged links also avoid turning a privacy promise into a
marketing exception.

## Anti-patterns

- Trying to trigger Trending or asking people to coordinate stars.
- Posting the same link and wording to many communities on the same day.
- Treating an open-source license as permission to ignore self-promotion rules.
- Launching broadly while installation still needs founder assistance.
- Leading a developer audience with procurement, enterprise licensing, or abstract governance.
- Claiming parity with a famous product when the user-visible boundary differs.
- Repeating a numerical hook without its conditions.
- Paying for stars, hidden endorsements, or editorial-looking placements.
- Adding many directories without an owner and a correction routine.
- Measuring success with stars while support, first use, and retention are failing.

## Ghostlight implications

Ghostlight already has several strong distribution primitives:

- a visually distinctive, ten-to-twelve-second real-browser hero;
- a one-sentence practitioner promise;
- a local, visible, authenticated-browser distinction that can be demonstrated;
- support for multiple MCP clients;
- an official MCP Registry entry and npm package;
- a concrete no-account, no-telemetry, no-activation story;
- governance and trust material for the second half of the journey;
- a decision aid for people comparing deployment shapes.

The largest current risk is conversion timing. The Chrome Web Store listing is still pending. A
manual unpacked-extension path is acceptable for a small, advanced feedback cohort, but broad
attention should wait until a stranger can install without founder guidance. The non-author review
already established that manual installation was understandable but cumbersome.

The recommended sequence is captured separately in
[the Ghostlight public-awareness plan](../design/public-awareness-plan-2026-07.md). It is a draft
work plan, not authorization to publish.

## Sources

### Primary platform and methodology sources

- [GitHub Open Source Guide: Finding Users for Your Project](https://opensource.guide/finding-users/)
- [GitHub: saving repositories with stars](https://docs.github.com/en/get-started/exploring-projects-on-github/saving-repositories-with-stars)
- [GitHub: viewing repository traffic](https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-traffic-to-a-repository)
- [Show HN Guidelines](https://news.ycombinator.com/showhn.html)
- [Lobsters About and Guidelines](https://lobste.rs/about)
- [Bluesky custom feeds](https://docs.bsky.app/docs/starter-templates/custom-feeds)
- [Bluesky search](https://bsky.social/about/blog/05-31-2024-search)
- [Product Hunt featuring guidelines](https://help.producthunt.com/en/articles/9883485-product-hunt-featuring-guidelines)
- [Product Hunt posting guide](https://help.producthunt.com/en/articles/479557-how-to-post-a-product)
- [r/LocalLLaMA 2026 rule update](https://www.reddit.com/r/LocalLLaMA/comments/1su3ao4/rlocalllama_rule_updates/)
- [Official MCP Registry quickstart](https://modelcontextprotocol.io/registry/quickstart)
- [Archived GitHub Release Radar repository](https://github.com/github/release-radar)
- [Open-source GitHub Trending Bluesky bot](https://github.com/kawamataryo/bsky-github-trending-bot)

### Case sources

- [Hallmark repository](https://github.com/Nutlope/hallmark)
- [Hallmark creator announcement](https://www.linkedin.com/posts/nutlope_announcing-hallmark-an-open-source-design-activity-7462523734868795392-oWjC)
- [Destructive Command Guard repository](https://github.com/Dicklesworthstone/destructive_command_guard)
- [Destructive Command Guard creator explanation](https://www.linkedin.com/posts/jeffreyemanuel_agent-coding-life-hack-im-100-convinced-activity-7421442482082660352-l5AG)
- [pi-computer-use repository](https://github.com/injaneity/pi-computer-use)
- [Wigolo repository](https://github.com/KnockOutEZ/wigolo)
- [TurboVec repository](https://github.com/RyanCodrai/turbovec)
- [OpenCut repository](https://github.com/OpenCut-app/OpenCut)
- [OpenCut Hacker News discussion](https://news.ycombinator.com/item?id=44553752)
- [OpenCut in an active alternative discussion](https://www.reddit.com/r/CapCut/comments/1kuehto/opensource_alternative_to_capcut/)
- [Research on suspected fake GitHub stars](https://www.cs.cmu.edu/news/2025/fake-github-stars)
