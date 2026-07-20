# Getting an open-source project out and about

An end-to-end playbook for helping useful open-source work become discoverable, understandable,
adopted, and sustained.

This guide applies to libraries, command-line tools, end-user applications, self-hosted services,
plugins, developer infrastructure, research software, datasets, models, hardware, specifications,
and educational projects. Use the parts that fit the project's risk, maturity, and maintainer
capacity. A weekend utility does not need the release operation of a cryptographic library. Both
benefit from telling the truth clearly and making the first useful result easy.

This is publication guidance, not legal, tax, compliance, or security advice.

## The short version

Open-source distribution is not one launch post. It is a chain:

```text
truth -> proof -> first success -> relevant people -> conversation
      -> adoption -> recommendation -> amplification -> stewardship
```

A project grows when the right people can:

1. recognize that it solves a problem they have;
2. understand how it differs from familiar options;
3. verify the claim without taking a large risk;
4. reach a useful result quickly;
5. get help and judge whether the project is maintained;
6. explain it accurately to someone else.

Trending pages, roundup bots, directories, and newsletters normally appear late in this chain.
They amplify activity that already exists. They are not substitutes for useful software,
relevant communities, or good onboarding.

If time is scarce, do these ten things:

- choose a real open-source license and put it in `LICENSE`;
- remove secrets, private history, and material you cannot publish;
- write one sentence naming the user, problem, and difference;
- make the README show installation and one successful use near the top;
- publish through the package, app, plugin, hardware, or research registry users already search;
- test the public instructions on a clean environment with a person who did not write them;
- give users a support path and security reporters a private path;
- show one honest proof that fits the project;
- introduce it first where people already experience the problem;
- measure successful use and returning participation, not stars alone.

## 1. Decide what publication is for

Projects get distorted when promotion begins before the maintainer knows which outcome matters.
Choose one primary outcome for the next publication cycle.

| Goal | What success looks like | Best first audience |
|---|---|---|
| Validate the problem | People recognize the problem and attempt the workflow | Five to ten high-fit prospective users |
| Find users | Successful installs and repeated use | The ecosystem where the tool naturally belongs |
| Find contributors | Useful issues, documentation improvements, and bounded pull requests | Existing users plus adjacent maintainers |
| Establish a standard | Independent implementations, review, and interoperability | Domain experts and implementers |
| Improve research reproducibility | Others can obtain data, run the method, and reproduce results | Researchers and research software engineers |
| Build an integration ecosystem | Plugins, adapters, examples, and dependents appear | Platform developers and integrators |
| Attract institutional adoption | Evaluation, security review, and deployment pilots begin | Technical decision makers after practitioner proof |
| Fund maintenance | Sponsors understand the public value and maintenance burden | Existing beneficiaries, not cold traffic |
| Recruit maintainers | Trusted repeat contributors take ownership of defined areas | The current contributor community |

Do not ask one artifact to achieve all of these. A README that converts a new user is different
from a governance document that recruits a maintainer. A research abstract is different from an
installation tutorial. Give each audience a clear next step.

### Pick the publication posture

Name the project's actual maturity:

- **Experiment:** evidence and source are public; interfaces and results may change.
- **Preview or alpha:** outsiders can try it; breakage and missing coverage are expected.
- **Beta:** the main use is viable; feedback is sought before stability promises.
- **Stable:** compatibility, security response, and release expectations are documented.
- **Mature:** adoption is broad enough that governance, succession, and change management matter.
- **Maintenance mode:** supported scope is narrow and new features are not a goal.
- **Archived:** no maintenance is expected; alternatives and migration notes are provided.

The version number, README language, package metadata, and announcement should agree. "Production
ready" is a support promise, not a synonym for confidence.

Choose a versioning policy that matches the public contract. If the project uses
[Semantic Versioning](https://semver.org/), define the public API whose compatibility the version
describes. For applications, data, models, hardware, and standards, calendar versions, revision
numbers, editions, or another domain convention may be clearer. Whatever the scheme, do not replace
the contents of an already published version silently.

## 2. Know what kind of project you are publishing

Different project types need different proof and distribution surfaces.

| Project type | Primary user question | Best proof | Natural distribution | Adoption evidence |
|---|---|---|---|---|
| Library or SDK | Can I integrate this safely and keep it updated? | Minimal code example, API docs, compatibility table, tests | Language package registry | Dependents, imports, version retention, issue quality |
| CLI or developer tool | Will this save time in my environment? | Terminal recording, before/after task, one-command install | Package registry and OS package managers | Current-release downloads, repeat use reports, packaged distributions |
| Desktop or mobile app | Can I accomplish the task comfortably and trust the app? | Short visual story, screenshots, accessible onboarding | App stores, signed releases, package managers | Active installs where available, repeat releases, user workflows |
| Self-hosted service | Can I deploy, operate, upgrade, and recover it? | Reproducible deployment, resource profile, backup/restore test | Container and infrastructure registries | Running deployments, upgrade reports, integrations, operator questions |
| Plugin or extension | Does it fit the host and survive host updates? | Host-native demo, permission explanation, compatibility matrix | Official host marketplace or plugin registry | Installs, active versions, host compatibility, reviews |
| Framework | Does its model make a class of work easier? | Complete small application, architecture explanation, migration path | Language registries, templates, ecosystem catalogs | Third-party projects, plugins, teaching material, maintainers |
| Infrastructure component | Is it reliable under realistic load and failure? | Reproducible benchmarks, failure tests, operating guide | Artifact registries and infrastructure catalogs | Production reports, downstream packages, security reviews |
| Security tool | What threat does it cover, miss, and create? | Threat model, test corpus, false-positive/negative discussion | Security communities and package registries | Independent evaluation, disclosures handled, integrations |
| Research software | Can I reproduce the result and cite the work? | Environment lock, data provenance, notebook or script, DOI | Domain repositories, package registries, archival repositories | Citations, reproductions, derivative experiments |
| Dataset | May I use it, and what does it represent or omit? | Datasheet, samples, collection method, bias and license statement | Data repositories and domain catalogs | Downloads, citations, downstream analyses, corrections |
| Model or weights | What license, training provenance, evaluation, and limits apply? | Model card, evaluation protocol, resource needs, failure cases | Model hubs and archival repositories | Derivatives, benchmark reproductions, responsible deployments |
| Open hardware | Can I fabricate, source, assemble, and repair it? | BOM, CAD/source formats, build photos, test procedure | Hardware repositories, maker communities, certification catalogs | Independent builds, revisions, suppliers, field reports |
| Specification or protocol | Can independent parties implement the same behavior? | Conformance examples, test vectors, reference implementations | Standards forums, domain communities, versioned site | Independent implementations, interop reports, issue resolution |
| Educational resource | Can the intended learner complete it and know they succeeded? | Sample lesson, outcomes, exercises, answer or assessment model | Educational catalogs, community forums, static site | Completions, adaptations, translations, teaching reports |

For a hybrid project, choose one primary entry. A database with a desktop inspector should not make
a new reader decide which product it is before they understand either.

## 3. Define the people, problem, and promise

### Map audiences by job, not demographics

For each important audience, complete:

```text
Person:
Situation:
Job they are trying to complete:
Current workaround:
Cost or frustration:
Why this project is meaningfully different:
Evidence they will trust:
First useful result:
Reason they may reject it:
Where they already ask for help or discover tools:
```

Useful audience categories include:

- direct users;
- developers integrating the project;
- operators deploying it;
- contributors changing it;
- security or compliance reviewers;
- educators and explainers;
- package and distribution maintainers;
- organizations funding or depending on it.

Do not lead with the least frequent audience merely because it has the largest budget.

### Write the one-sentence project truth

Use this structure:

> For [specific user] who needs [recognizable job], [project] is a [honest category] that
> [meaningful outcome]. Unlike [current approach], it [defensible difference].

Then remove whatever is not necessary. Good short forms usually retain:

- the audience or context;
- the outcome;
- the category;
- one difference.

Avoid "next-generation," "revolutionary," "blazing fast," "enterprise grade," "secure," and
"production ready" unless the page immediately supplies a definition and proof.

### Build a claim ledger

Before publication, list every claim likely to appear in a README, post, talk, or directory.

| Claim | Evidence | Conditions | Known exceptions | Canonical source | Owner |
|---|---|---|---|---|---|
| Faster than X | Reproducible benchmark | Hardware, dataset, version | Small inputs | Benchmarks page | Maintainer A |
| Works on Y | Clean test result | Supported versions | Feature Z absent | Compatibility page | Maintainer B |
| Private/local | Architecture and network test | Default configuration | Optional remote feature | Security page | Maintainer A |

Portable claims travel furthest and lose nuance fastest. Put the condition beside the number.
Publish explicit non-claims for the mistakes a reasonable reader is likely to make.

## 4. Make the project legally publishable

Do this before attracting attention.

- Confirm the authors have the right to publish the code, data, documentation, media, models,
  hardware files, and trademarks.
- Check employer, university, client, grant, and contributor agreements.
- Remove credentials, personal data, customer material, internal URLs, private issue references,
  proprietary test fixtures, and generated artifacts containing secrets from both the tree and
  relevant history.
- Inventory third-party code, assets, fonts, datasets, models, and examples. Preserve required
  notices and source offers.
- Choose a license consistent with the
  [Open Source Definition](https://opensource.org/osd) if the project is being described as open
  source. Put the complete text in a prominent `LICENSE` file and use the correct
  [SPDX identifier](https://spdx.org/licenses/) in package metadata where supported.
- License documentation, datasets, model weights, and hardware designs explicitly when the
  software license does not clearly cover them.
- Decide whether contributions use the repository license, a Developer Certificate of Origin,
  or a contributor agreement. Do not introduce a CLA casually; explain why it is needed.
- Search the project name across source hosts, package registries, app stores, domain names, and
  relevant trademark databases before building recognition around it.
- If the project is a fork, credit upstream prominently and state why the fork exists, how far it
  has diverged, whether changes flow back, and which compatibility promises remain. Do not imply
  upstream endorsement.
- State whether logos and project names have separate trademark rules.
- If cryptography, sanctions, controlled data, health, finance, children, or regulated devices are
  involved, get qualified advice for the affected jurisdictions.

"Source available" and "open source" are not interchangeable. A license that prohibits fields of
use, commercial activity, or redistribution may be a valid source-available choice, but it should
not be promoted as open source.

## 5. Build the canonical home

Every public mention should lead to one canonical place. That can be the repository or a project
site, but it must let a newcomer find source, documentation, releases, security information, and
support without guessing.

### Repository metadata

Complete:

- clear repository description;
- canonical homepage;
- relevant topics/tags;
- social preview image;
- license detection;
- release and package links;
- short project name and consistent namespace;
- archived/fork/template status set accurately.

Search engines, package indexes, social previews, code hosts, and automatic curators reuse this
metadata. Treat it as a public API for discovery.

### README reader order

A useful default order is:

1. name and one-sentence outcome;
2. proof artifact;
3. who it is for and who it is not for;
4. shortest supported install;
5. first useful example and expected result;
6. core capabilities;
7. supported platforms, versions, and limits;
8. documentation and support;
9. architecture or technical depth;
10. contribution path;
11. security, license, governance, funding, and citation.

Do not make the reader cross a manifesto, badge wall, company story, funding request, or complete
API reference before seeing what the project does.

### Community health files

At minimum, consider:

- `README.md`: user and contributor orientation;
- `LICENSE`: permissions and obligations;
- `CONTRIBUTING.md`: setup, tests, scope, review, and decision process;
- `CODE_OF_CONDUCT.md`: behavior and enforcement contacts, if maintainers can enforce it;
- `SECURITY.md`: supported versions and private reporting instructions;
- `SUPPORT.md`: where usage questions, bugs, and discussions belong;
- issue forms/templates: enough structured evidence to reproduce a report;
- pull request template: tests, compatibility, documentation, and security impact;
- `GOVERNANCE.md`: roles, decisions, conflicts, and succession when more than one maintainer or
  institution has authority;
- `CHANGELOG.md` or equivalent release history;
- [`CITATION.cff`](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-citation-files):
  preferred citation for research and broadly reused technical work;
- `FUNDING.yml`: funding paths, only when they are ready to receive support.

[GitHub's community profile](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/about-community-profiles-for-public-repositories)
checks for several of these files. A perfect checklist is not community health by itself. A code of
conduct without a confidential contact or willingness to enforce it can be worse than an honest,
limited process.

## 6. Make first use survive contact with a stranger

### Design the activation path

Activation is the first moment the user receives the promised value. Define it precisely:

```text
Starting state:
Install action:
Required configuration:
First command or gesture:
Expected visible/output result:
Time on a normal connection and machine:
Clean uninstall or rollback:
Recovery command or document:
```

Then remove steps, not explanations. A short but magical installer that makes unexplained system
changes is not better onboarding.

### Test clean environments

Test the exact public path on:

- a new account or logged-out browser;
- a clean operating system, container, VM, device, or profile;
- supported minimum and current versions;
- paths containing spaces and non-English characters where relevant;
- a network without maintainer-local caches or credentials;
- the least privileged account the project claims to support;
- install, upgrade, downgrade where supported, and uninstall;
- both success and the most likely failure.

Ask a non-author to follow only the published instructions. Do not rescue them immediately. Record
where they hesitate, what they think will happen, and whether recovery language is actionable.

### Respect packaging norms

Publish where the user already searches:

- language library -> its language registry;
- CLI -> language registry plus appropriate OS package managers;
- containerized service -> container registry and deployment catalog;
- editor, browser, or platform plugin -> official marketplace;
- mobile/desktop app -> relevant stores and signed releases;
- model -> model hub;
- dataset/research artifact -> archival or domain repository with a stable identifier;
- hardware -> source repository plus BOM/CAD release and relevant open-hardware catalog;
- standard -> versioned specification site, issue process, and conformance assets.

The source repository is canonical evidence, but it is not always the user's installation surface.

## 7. Choose proof that fits the claim

### Proof forms

| Proof form | Best for | Requirements |
|---|---|---|
| Ten-to-thirty-second visual story | Apps, plugins, visual tools, workflows | Show the real product, readable at normal speed, captioned |
| Terminal recording | CLI, build, deployment, migration | Include command, result, timing context, and copyable text nearby |
| Minimal code sample | Libraries, SDKs, protocols | Complete, runnable, versioned, and small enough to understand |
| Before/after artifact | Formatters, optimizers, design or migration tools | Use representative inputs and disclose manual cleanup |
| Reproducible benchmark | Performance and resource claims | Publish code, versions, hardware, data, warmup, variance, and tradeoffs |
| Compatibility matrix | Integrations and cross-platform work | Name tested versions and the meaning of full/partial support |
| Failure or recovery demonstration | Reliability, backup, security, orchestration | Show the failure injection and verified recovery outcome |
| Independent reproduction | Research, standards, security, infrastructure | Link method and discrepancies, not only praise |
| Case study | Mature operational projects | Name scale, duration, constraints, and who verified the result |
| Test vectors/conformance suite | Protocols and formats | Version them and separate normative behavior from implementation |
| BOM and build log | Hardware | Include exact revisions, substitutions, tools, safety notes, and test |
| Datasheet/model card | Data and models | Provenance, intended use, exclusions, bias, evaluation, and license |

### Proof hygiene

- Use the current public release, not a maintainer-only build.
- Make text alternatives available for video, GIFs, screenshots, and diagrams.
- Add captions and transcripts; avoid rapid cuts and flashing.
- Do not hide the browser chrome, terminal setup, manual step, or failure that materially changes
  the viewer's understanding.
- Separate measured fact, user report, and maintainer inference.
- Keep a reproducibility date and version on benchmarks.
- Archive the proof input so later releases can be compared honestly.

### Accessibility and global reach

- Supply alt text for meaningful images and diagrams.
- Caption video and provide a transcript or equivalent written walkthrough.
- Keep commands, code, and URLs available as text instead of embedding them only in media.
- Avoid flashing, rapid cuts, tiny terminal text, and color-only status distinctions.
- Use plain language and explain community-specific acronyms.
- Design documentation for narrow screens and constrained connections where practical.
- Publish UTC times beside local event times.
- Prefer asynchronous feedback paths so participation does not require one time zone.
- Accept documentation and translation work as first-class contributions.
- Translate the stable orientation and install path before translating a fast-changing reference.
- Assign an owner to each maintained translation and state when a translation lags the canonical
  language.
- Learn the words people use for the problem in their community or language; do not mechanically
  translate promotional copy and assume the same framing travels.

## 8. Establish trust before asking for reach

The amount of trust work should scale with what the project can affect.

### Baseline for most software

- CI runs tests on supported environments.
- Releases come from a documented process.
- Maintainer accounts use strong authentication.
- Branch and release authority is limited.
- Dependencies and generated files are reviewable.
- `SECURITY.md` names supported versions and a private report path.
- Known limitations and data behavior are public.
- Artifacts use HTTPS and checksums at minimum.

GitHub public repositories can enable
[private vulnerability reporting](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/configure-for-a-repository)
so researchers do not have to disclose details in an issue.

### Higher-impact projects

Depending on risk, add:

- threat model and trust boundaries;
- signed releases and verifiable provenance;
- software bill of materials;
- dependency pinning and update policy;
- static analysis, fuzzing, and sanitizer coverage;
- protected release environments and separation of build/publish duties;
- vulnerability disclosure and advisory process;
- reproducible or independently verifiable builds;
- support and end-of-life policy;
- disaster recovery and maintainer succession;
- independent review or audit.

Use [OpenSSF Scorecard](https://scorecard.dev/), the
[OpenSSF Best Practices Badge](https://openssf.org/projects/best-practices-badge/), and the
[OpenSSF OSPS Baseline](https://baseline.openssf.org/) as structured inputs. Do not treat a badge
or aggregate score as a substitute for a threat model or project-specific judgment.

### Trust language

Say:

- what the project does;
- which versions and environments were tested;
- what data crosses which boundary;
- who controls updates and releases;
- which threats are addressed;
- which threats and use cases are outside scope.

Avoid absolute "safe," "secure," "private," "anonymous," or "zero trust" claims.

## 9. Build a community surface maintainers can sustain

Every new user creates possible questions, bugs, feature requests, support load, moderation, and
security work. Publication without a response plan converts success into maintainer debt.

### Separate conversation types

| Need | Appropriate surface |
|---|---|
| Reproducible bug | Issue tracker |
| Feature proposal | Discussion or structured issue |
| Usage question | Discussion, forum, Q&A, or chat if staffed |
| Security vulnerability | Private reporting path |
| Time-sensitive paid support | Separate commercial channel if offered |
| Governance decision | Public proposal/RFC process |
| Informal community exchange | Chat or forum with moderation |

Write down where each belongs. Do not create Discord, Slack, Matrix, a forum, Discussions, and a
mailing list merely because successful projects have them. Empty surfaces fragment knowledge.

### Set expectations

State:

- whether support is best effort;
- typical review cadence without promising an SLA you cannot meet;
- what environments are supported;
- which feature requests are out of scope;
- how stale reports are handled;
- how decisions are made;
- how maintainers take breaks.

### Prepare contribution units

New contributors succeed when work is bounded and the acceptance path is visible. Prepare:

- documentation corrections;
- small test additions;
- examples for a supported use case;
- translations with an owner and update process;
- packaging for a specific distribution;
- labeled issues with setup hints and likely files;
- triage and reproduction tasks that do not require commit access.

Do not label a complex architectural problem "good first issue" to attract labor.

The [Open Source Guides community guidance](https://opensource.guide/building-community/) treats
the path from user to contributor to maintainer as a funnel. Respecting the first contribution is
part of distribution: contributors become the most credible explainers of a project they trust.

## 10. Find the first people

### Start with problem proximity

The first audience is rarely "developers" or "the open-source community." Look for:

- the host platform or language ecosystem;
- issue trackers and discussions where the pain is named;
- professional or research communities using the current workaround;
- package, app, model, data, hardware, or plugin registries;
- maintainers of adjacent tools;
- people who asked for an alternative;
- educators already teaching the problem;
- organizations publicly depending on the underlying workflow.

Search using the user's words, not the project's category:

```text
"how do I ..."
"alternative to ..."
"X is too slow"
"X deleted/lost/broke ..."
"need a local/offline/accessible ..."
"library for ..."
"can't deploy ..."
```

Do not enter old conversations merely to drop a link. Answer the question fully, disclose your
relationship, and mention the project only where it is genuinely relevant.

### Run a proof cohort

Invite five to ten people who represent different environments or roles. Give them only public
materials. Ask:

1. What did you think the project would do before installing?
2. Where did you hesitate?
3. Did you reach the promised first result?
4. What surprised or worried you?
5. How would you explain it to a colleague?
6. What would make you return next week?

Look for repeated failures, not average sentiment. Three people misunderstanding the same boundary
is a documentation or positioning defect even if they say they liked the project.

## 11. Design the publication campaign

### Use one anchor and several native adaptations

The anchor is the canonical, discussable publication event:

- a release and repository launch;
- Show HN;
- a technical article;
- a research paper and artifact release;
- a host-platform showcase;
- a major interoperability demonstration;
- a conference talk with a runnable release.

Other channels should adapt the same facts to their audience. Do not paste the same launch copy
everywhere.

### Channel families

| Family | Examples | Best use | Failure mode |
|---|---|---|---|
| Owned | Repository, site, docs, changelog, mailing list | Canonical truth and conversion | Beautiful announcement, weak onboarding |
| Registries | npm, PyPI, crates.io, Maven, NuGet, app/plugin/model/container catalogs | High-intent discovery and install | Incomplete metadata or abandoned versions |
| Host ecosystems | Platform forums, showcases, integration catalogs, community calls | Exact-fit users | Hijacking support channels |
| Technical aggregators | Hacker News, Lobsters, language news sites | Deep discussion and broad developer discovery | Broadcasting without founder participation |
| Problem communities | Domain forums, relevant subreddits, practitioner groups | Native problem/solution discussion | Self-promotion without community history |
| Social networks | Bluesky, Mastodon, LinkedIn, X | Visual proof, founder story, relationships | Empty impressions and context collapse |
| Durable media | Project blog, DEV, YouTube, podcasts | Searchable explanation and education | Content that adds no value beyond a link |
| Curated media | Newsletters, roundup writers, press, maintainers' lists | Earned second wave | Distorted claims or paid editorial disguise |
| Events | Meetups, conferences, workshops, office hours | Trust, depth, and live feedback | Talk before users can try the project |
| Institutional | Foundations, academic venues, standards bodies, OSPO catalogs | Longevity, review, adoption legitimacy | Process without practitioner value |
| Direct outreach | Adjacent maintainers, known users, organizations with public need | Specific feedback or integration | Mass cold mail and implied endorsement |

### Platform etiquette

- Read the current rules immediately before posting.
- Disclose that you maintain or contribute to the project.
- Create a native artifact that is useful without clicking away.
- Ask a specific question instead of "thoughts?"
- Stay available for the active discussion.
- Do not ask friends, employees, or communities to coordinate votes or comments.
- Do not use alternate accounts, fake users, purchased stars, or undisclosed endorsements.
- Do not argue with moderators about a removed post. Learn and choose a better surface.
- Space adaptations so maintainers can respond and learn between them.

[Show HN](https://news.ycombinator.com/showhn.html) requires a project people can try and forbids
vote solicitation. [Lobsters](https://lobste.rs/about) expects authentic participation and treats
less than one quarter self-promotional activity as a rule of thumb. Reddit and other community
rules vary by community and change over time.

### Match the story to the channel

| Audience | Lead with | Evidence | Ask |
|---|---|---|---|
| User with a painful workflow | Outcome and time/risk removed | Short real workflow | Try one named task |
| Developer/integrator | API or architecture difference | Runnable example and docs | Integrate or challenge assumptions |
| Operator | Deployment, upgrade, observability, recovery | Operating recipe and failure test | Run a bounded pilot |
| Security reviewer | Threat, trust boundary, controls, limits | Security docs and artifacts | Review a defined boundary |
| Researcher | Question, method, artifact, reproducibility | Data/code/environment and DOI | Reproduce or extend |
| Hardware builder | Buildability and sourcing | BOM, CAD, build/test log | Build a specific revision |
| Potential contributor | Public need and bounded work | Contributor setup and labeled issue | Complete one clear contribution |
| Sponsor | Public value and maintenance work | Usage/community evidence and budget need | Fund work without implied control |

### Timing template

Use the sequence, not necessarily the duration.

#### Before announcing

- freeze the claim ledger and support matrix;
- test clean install and first success;
- publish release notes and artifacts;
- verify package/store/registry metadata;
- prepare support, moderation, and security response;
- archive measurement baselines;
- prepare accessible visual and text proof;
- check every public link logged out;
- identify who will answer questions.

#### Anchor day

- verify service and downloads before posting;
- publish one anchor;
- be present;
- record questions and failures verbatim;
- correct critical documentation immediately;
- avoid shipping risky code under attention pressure;
- do not manufacture engagement.

#### Next 72 hours

- triage bug, docs, compatibility, message, and feature feedback separately;
- publish known issues if several users hit the same problem;
- thank independent explainers and correct material errors;
- adapt to one or two high-fit channels;
- snapshot traffic and adoption signals.

#### Days 4 through 14

- publish a durable technical or educational artifact;
- make safe onboarding fixes;
- contact selected curators only if the project now has evidence;
- report what changed because of user feedback;
- compare results with the baseline;
- decide whether to continue, adapt, or pause.

#### Afterward

- announce substantive releases, not every patch as a relaunch;
- maintain registry and directory truth;
- cultivate users and contributors between promotion cycles;
- archive campaign findings for the next maintainer.

## 12. Write publication material people can repeat accurately

### The six-part launch brief

```text
1. Problem:
   A concrete situation the intended user recognizes.

2. Project:
   One sentence naming the category and outcome.

3. Difference:
   The smallest defensible contrast with the current approach.

4. Proof:
   One visual, example, benchmark, reproduction, or case.

5. First use:
   The shortest supported path to the promised result.

6. Invitation:
   One task to try or one question the maintainer wants answered.
```

Add boundaries immediately after, especially when the category invites unsafe or exaggerated
assumptions.

### Community post template

```text
Title: [specific outcome or problem, not a slogan]

I maintain [project]. I built it because [concrete problem/context].

It [one-sentence outcome]. The important difference from [familiar approach] is [difference].

Here is [proof that can be understood in this post].

You can try the smallest useful path with:
[command/link/steps]

Current limits: [two or three material boundaries].

I would especially value feedback from [specific people] on [specific question]. I will be here
to answer implementation and design questions.
```

### Curator or newsletter brief

```text
Subject: Open-source [category]: [specific outcome]

What it is:
Who it is for:
Why it is different:
Proof:
Install/try:
Source and license:
Current maturity:
Three non-claims:
Maintainer available for questions:
```

Send it only to a person or publication that covers the category. Personalize the reason it fits.
Do not attach a false deadline or ask for guaranteed coverage.

### Release note structure

```text
Outcome: what users can now do
Why: the problem or feedback behind it
Changes: concise grouped list
Compatibility: supported versions and breaking behavior
Upgrade: exact steps
Security: advisories or relevant changes
Known issues: honest current limitations
Artifacts: package/release links and verification
Contributors: credit
```

## 13. Measure the journey, not the applause

### Measurement layers

| Layer | Question | Signals |
|---|---|---|
| Discovery | Did the right people encounter it? | Unique visitors, search/referrers, qualified post views |
| Interest | Did they investigate? | Documentation depth, saves, stars, release-page visits, questions |
| Trial | Did they attempt first use? | Package/release downloads, sandbox runs, install questions |
| Activation | Did they receive the promised value? | First-success reports, completed example, created output |
| Retention | Did they return? | Repeat version adoption, recurring users where privacy permits, returning reporters |
| Integration | Did it become part of other work? | Dependents, plugins, citations, deployments, derivative builds |
| Community | Did participation deepen? | Repeat contributors, review activity, independent answers, maintainers |
| Trust | Is it understood and relied on accurately? | Independent reviews, correct descriptions, handled disclosures |
| Sustainability | Can maintainers continue? | Response load, funding, maintainer count, time-to-review, burnout signals |

Stars are useful bookmarks, appreciation, and discovery signals. They are not installations,
retention, security, or community health. Downloads may include CI, mirrors, updates, and bots.
Clone counts can be dominated by automation. Use multiple signals and state uncertainty.

### Establish a baseline

Before each meaningful publication event, archive:

- repository visitors, referrers, and popular pages;
- stars, watchers, forks, and dependents;
- current-release and package downloads;
- docs search terms and failure pages if collected ethically;
- issue/discussion volume and response capacity;
- contributor and release cadence;
- known install and activation success rate from proof users.

GitHub repository traffic retains only 14 days, so snapshots must be archived manually. For deeper
community analysis, use the [CHAOSS metrics models](https://chaoss.community/kb/metrics-model-starter-project-health/)
as a menu, not a universal scorecard.

### Privacy and metrics

- Collect the minimum data needed for a decision.
- Prefer aggregate public platform and package counters.
- Do not add product telemetry merely because promotion created a measurement question.
- If telemetry exists, document it, obtain appropriate consent, minimize retention, and give users
  meaningful control.
- Do not fingerprint users across package, site, social, and support surfaces.
- Treat issue authors and contributors as people, not conversion events.

### Campaign ledger

```text
Event:
UTC date/time:
Channel/community:
Public URL:
Audience:
Problem angle:
Proof:
Specific ask:

Baseline:
24 hours:
72 hours:
7 days:
14 days:

Questions:
Install/activation failures:
Misunderstandings:
Unexpected use cases:
Documentation/product changes:
Organic recommendations:
Maintainer load:
Decision: continue / adapt / pause
```

## 14. Turn feedback into decisions

Classify before acting:

| Feedback type | Example | Response |
|---|---|---|
| Product defect | Supported command fails | Reproduce, prioritize by impact, document workaround |
| Documentation defect | Correct behavior is hard to reach | Repair the path quickly |
| Positioning defect | Users expect a different category | Clarify message and non-claims |
| Compatibility gap | Unsupported platform is common | Record demand; do not imply immediate support |
| Feature request | User wants adjacent workflow | Ask for job/context before designing feature |
| Security report | Trust boundary may fail | Move private, acknowledge, follow disclosure process |
| Community/process issue | Contributor cannot understand acceptance | Improve CONTRIBUTING/governance |
| Out-of-scope use | Project is being stretched dangerously | Say no clearly and explain the supported alternative |
| Praise | "This is great" | Thank them; ask what worked only if useful and respectful |

Do not let the loudest launch-day comment become the roadmap. Look for repeated jobs, strategic
fit, maintainer capacity, risk, and evidence of actual use.

Publish what changed because of feedback. Closing the loop shows that participation matters.

## 15. Prepare for failure and unwanted attention

Attention can expose defects, dependency confusion, impersonation, malicious packages, security
reports, harassment, license disputes, and inaccurate press.

Before broad publication, decide:

- who can pause downloads or yank a compromised release;
- how users will be notified of a security issue;
- which accounts control domains, registries, social handles, and signing keys;
- how to prove the canonical package and site;
- who moderates abusive discussion;
- how to correct a false claim without amplifying it unnecessarily;
- what happens if the maintainer becomes unavailable;
- when the project will pause rather than keep promoting through a serious defect.

If a security issue appears:

1. move details to the private reporting channel;
2. acknowledge without promising an unverified timeline;
3. determine affected versions and exposure;
4. prepare and verify the fix privately where possible;
5. publish an advisory and fixed release;
6. credit the reporter according to their preference;
7. update the process after the incident.

Never suppress a legitimate report to protect a launch.

## 16. Make funding honest and non-coercive

Funding is part of sustainability, not proof that users owe the project money.

Possible models include:

- donations and sponsorships;
- grants;
- employer-funded maintenance;
- foundation or institutional support;
- paid support, training, implementation, or certification;
- hosted or managed services;
- dual licensing or separately licensed additions;
- bounties for well-defined work.

State:

- who receives funds;
- what they support;
- whether benefits or tiers exist;
- whether donations affect governance or roadmap priority;
- the license boundary;
- how conflicts of interest are handled.

Donations can simply earn gratitude. If so, say so. Do not imply that a donation buys support,
access, influence, or a feature unless that is an explicit offering.

GitHub can display funding options through
[`FUNDING.yml`](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/displaying-a-sponsor-button-in-your-repository).
Add it after the receiving account, tax/bank setup, ownership, and public explanation are ready.

## 17. Protect maintainer capacity

Growth is not success if it makes the project impossible to maintain.

- Limit the number of channels you promise to monitor.
- Prefer searchable, public answers over repeating private support.
- Create saved responses for common routing, not dismissive automation.
- Batch triage and reviews.
- Label maintenance mode and response expectations honestly.
- Recruit maintainers through repeated trust, not urgency.
- Give documentation, moderation, design, translation, release, and community work equal credit.
- Take breaks and publish them if users need to adjust expectations.
- Archive or transfer projects responsibly when interest is gone.

The [Open Source Guides maintainer guide](https://opensource.guide/best-practices/) treats maintainer
happiness as necessary for project survival. A publication plan should include a support budget in
hours, not only a hoped-for audience size.

## 18. Decision trees

### Is the project ready for broad publication?

```text
Is there a real open-source license and publication authority?
  no -> stop and resolve rights/license
  yes
   |
Can a stranger obtain it and reach the promised result?
  no -> run a proof cohort and repair onboarding
  yes
   |
Are material limits, supported environments, security reporting, and support paths public?
  no -> publish the missing truth
  yes
   |
Can maintainers answer questions and handle a serious defect during the launch window?
  no -> reduce scope, choose a later date, or soft launch
  yes
   |
Is there a specific audience and one proof they will trust?
  no -> research problem communities and build proof
  yes -> choose an anchor and publish in stages
```

### Which channel should come first?

```text
Does the project live inside a host ecosystem or registry?
  yes -> publish there first
  no
   |
Is the main value technically interesting and directly runnable?
  yes -> consider Show HN or a technical aggregator
  no
   |
Is there an active community organized around the exact problem?
  yes -> participate there with a native problem-first artifact
  no
   |
Is the result visual or experiential?
  yes -> lead with accessible video/social proof and a canonical try path
  no
   |
Is the work research, hardware, or a standard?
  yes -> lead through its domain venue plus reproducible artifact
  no -> begin with direct high-fit proof users and learn where they gather
```

### Attention arrived but adoption did not

```text
Qualified visitors increased?
  no -> wrong channel, weak title, or no distribution
  yes
   |
Trials/downloads increased?
  no -> unclear value, trust gap, or install friction
  yes
   |
First success increased?
  no -> activation defect or wrong audience
  yes
   |
Return/integration increased?
  no -> weak ongoing value, reliability, or support
  yes -> cultivate users, contributors, and earned amplification
```

## 19. Minimum, standard, and high-impact tracks

### Minimum responsible publication

Good for a low-risk hobby tool or example:

- license and rights checked;
- secrets removed;
- clear README with install, example, limits, and support status;
- source tag or release;
- one clean-path test;
- one relevant registry/community introduction;
- no promise of support beyond capacity.

### Standard maintained project

Add:

- CI and support matrix;
- package/registry distribution;
- changelog and version policy;
- CONTRIBUTING, Code of Conduct, SECURITY, issue templates;
- proof cohort;
- accessible proof artifact;
- staged publication and campaign ledger;
- dependency and release security;
- periodic community and maintenance review.

### High-impact or security-sensitive project

Add:

- documented governance and succession;
- threat model and private vulnerability workflow;
- signed/provenanced artifacts, SBOM, protected publishing;
- independent review and reproducible tests/benchmarks;
- support and end-of-life policy;
- incident and notification plan;
- legal/compliance review appropriate to the domain;
- measured broad launch with staffed response;
- ongoing OpenSSF/CHAOSS-informed health review.

## 20. Master checklist

### Purpose

- [ ] Primary publication outcome chosen
- [ ] Maturity posture named accurately
- [ ] Primary audience and first useful result defined
- [ ] Maintainer capacity and launch owner confirmed

### Rights and identity

- [ ] Publication rights verified
- [ ] Open-source license selected and included
- [ ] Third-party notices and asset/data/model/hardware licenses handled
- [ ] Secrets and private material removed from tree and relevant history
- [ ] Name, namespace, domain, package, and trademark conflicts checked

### Product and proof

- [ ] One-sentence truth written
- [ ] Claim ledger complete
- [ ] Material non-claims stated
- [ ] Install, upgrade, and uninstall tested where relevant
- [ ] First useful example works on a clean environment
- [ ] Proof matches the project type and is reproducible/accessible
- [ ] Supported platforms/versions and limits are clear

### Repository and distribution

- [ ] Description, homepage, topics, and social preview complete
- [ ] README follows newcomer order
- [ ] Package/store/registry metadata agrees with the repository
- [ ] Releases and artifacts are versioned and verifiable
- [ ] Canonical home, docs, source, and package link to one another

### Trust and community

- [ ] SECURITY and private reporting path ready
- [ ] Support and issue-routing paths ready
- [ ] Contribution process and bounded tasks ready
- [ ] Code of Conduct contact and enforcement capacity ready
- [ ] Governance and succession match project maturity
- [ ] Maintainer accounts and release authority protected
- [ ] Incident, moderation, and correction owners named

### Publication

- [ ] Proof cohort completed
- [ ] Baseline archived
- [ ] Anchor artifact prepared
- [ ] Community rules rechecked
- [ ] Affiliation disclosed
- [ ] Native adaptations prepared, not syndicated blindly
- [ ] Maintainers available during active discussion
- [ ] No vote coordination, fake accounts, bought metrics, or hidden placement

### After publication

- [ ] Feedback classified
- [ ] Repeated onboarding defects repaired
- [ ] Known issues published
- [ ] 24-hour, 72-hour, 7-day, and 14-day snapshots archived
- [ ] Third-party descriptions checked for material errors
- [ ] Contributors and reporters credited
- [ ] Continue/adapt/pause decision recorded
- [ ] Maintenance and funding load reviewed

One unchecked box does not always block publication. Know why it is unchecked, who bears the risk,
and whether the public description remains honest.

## 21. Anti-patterns

- Launching to compensate for an unclear problem.
- Treating "open source" as the audience.
- Confusing source availability with an open-source license.
- Optimizing for GitHub Trending, stars, or votes.
- Posting identical copy to many communities at once.
- Asking employees, friends, or users to manufacture engagement.
- Publishing benchmarks without method, conditions, or tradeoffs.
- Calling a preview stable to improve conversion.
- Hiding a required account, cloud path, paid boundary, permission, or manual step.
- Creating more community channels than maintainers can support.
- Using an issue tracker as unpaid customer support without scope or boundaries.
- Treating first contributors as a free feature backlog.
- Shipping risky changes during launch pressure without normal review.
- Adding telemetry silently to answer a marketing question.
- Buying directory placement or editorial coverage without disclosure.
- Announcing every patch as a new launch.
- Allowing stale directories and mirrors to become the apparent canonical source.
- Ignoring a security report to protect momentum.
- Making funding benefits or governance influence ambiguous.
- Growing past maintainer capacity and calling the resulting backlog success.

## 22. Primary references

- [Open Source Guides: Starting an Open Source Project](https://opensource.guide/starting-a-project/)
- [Open Source Guides: Finding Users](https://opensource.guide/finding-users/)
- [Open Source Guides: Building Welcoming Communities](https://opensource.guide/building-community/)
- [Open Source Guides: Best Practices for Maintainers](https://opensource.guide/best-practices/)
- [Open Source Guides: Metrics](https://opensource.guide/metrics/)
- [GitHub community profiles](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/about-community-profiles-for-public-repositories)
- [GitHub repository traffic](https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-traffic-to-a-repository)
- [GitHub security policies and private reporting](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting)
- [GitHub repository sponsor buttons](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/displaying-a-sponsor-button-in-your-repository)
- [SPDX License List](https://spdx.org/licenses/)
- [Open Source Initiative: Open Source Definition](https://opensource.org/osd)
- [Semantic Versioning 2.0.0](https://semver.org/)
- [GitHub citation files](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-citation-files)
- [Open Source Hardware Association definition](https://oshwa.org/definition/)
- [Open Source Hardware Association sharing practices](https://oshwa.org/resources/sharing-best-practices/)
- [OpenSSF Scorecard](https://scorecard.dev/)
- [OpenSSF Best Practices Badge](https://openssf.org/projects/best-practices-badge/)
- [OpenSSF OSPS Baseline](https://baseline.openssf.org/)
- [CHAOSS Starter Project Health Metrics](https://chaoss.community/kb/metrics-model-starter-project-health/)
- [Show HN Guidelines](https://news.ycombinator.com/showhn.html)
- [Lobsters Guidelines](https://lobste.rs/about)

## Research provenance

This guide distills the general findings and case studies in
[Open-source publication paths, 2026-07](../research/20-open-source-publication-paths-2026-07.md).
The research found that successful projects used different channels but shared the same underlying
path: a legible problem, trustworthy proof, easy first use, high-affinity seed users, and later
earned amplification. Platform rules and services change. Recheck them before each campaign.
