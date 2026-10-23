# Windows code signing for Ghostlight -- research, 2026-10-23

Asked by the owner after WinGet's 2.0.1 validation was blocked by a Defender machine-learning verdict
(`Trojan:Script/Wacatac.C!ml`, microsoft/winget-pkgs PR 458539). Research only: nothing was bought,
signed up for or posted. eMudhra was excluded at the owner's request and is not assessed. This is
an input to a decision, not the decision: docs/MEMORY.md, docs/RELEASE.md,
docs/RELEASE-CHECKLIST.md and docs/trust/security-overview.md state that Ghostlight carries no
Authenticode certificate, and changing that takes an owner decision (an ADR or amendment).

## Recommendation

**Primary: Azure Artifact Signing (formerly Trusted Signing), Basic, validated as an individual**
(or as the organization "Sylin" only if Sylin is a registered legal entity).

- The only route Ghostlight qualifies for today: Microsoft accepts individual developers in the US
  or Canada. $9.99 a month (about $120 a year), no hardware token, no yearly certificate renewal.
- Microsoft's official GitHub Action runs on the `windows-2025` runner `release.yml` already uses,
  logs in with OIDC (no long-lived secret) and needs no human step. Signing before the checksums,
  the npm launcher, the .mcpb, the WinGet manifest and the attestations makes all of them cover the
  signed bytes.
- Microsoft calls it its recommended code signing service outside the Store.
- It matters most for Smart App Control: in enforcement mode Windows 11 blocks unknown, unsigned
  code whatever installed it (npm, WinGet, the PowerShell one-liner, or Chrome starting the native
  host). A signed file with no other verdict is allowed.

**Fallback: an SSL.com IV certificate with eSigner cloud signing**, about $309 a year ($129 for the
certificate plus $180 for eSigner Tier 1). Official GitHub Action; the owner's name on the
certificate. Use it if Azure validation fails or the account is suspended. Cheaper alternative:
Certum Open Source (about EUR 49 a year), but its cloud signing has no official headless CI
login; CI means scripting its 2FA.

**Revisit later: SignPath Foundation (free).** In September and October 2026 it declined many small
projects for lack of public visibility; Ghostlight's repository dates from 2026-07-01 and has
1 star, 0 forks, about 1,465 npm downloads a month and a few hundred GitHub downloads a release. It
also needs a manual approval per release and shows "SignPath Foundation" as the publisher.

**In every case:** keep submitting false positives to Microsoft (WDSI) as a software developer,
and submit each new Windows zip before opening its WinGet PR. Signing helps but guarantees nothing
against `!ml` verdicts: Microsoft says consistent signing "helps our research team" and may get a
program onto its known list, and it does not take allow-list requests from developers. No
before-and-after data was found that signing alone clears `!ml` verdicts. The same
`Trojan:Script/Wacatac.C!ml` verdict hit another unsigned project's WinGet zip (repo-keeper, Go)
on 2026-10-21, which looks like a model-wide sweep rather than something specific to Ghostlight.

## Comparison

| Route | Eligible now? | Cost a year | Trusted by Windows? | Publisher shown | CI; human step? | Limits and validity | Vetting | Verdict |
|---|---|---|---|---|---|---|---|---|
| Azure Artifact Signing, individual | Yes (US individual) | about $120 ($9.99/mo, not pro-rated, paid Azure subscription) | Yes, Microsoft ID Verified chain; see the note below | Owner's legal name, city, state, country | Official action, Windows runner, OIDC; none | 5,000 signatures/mo; 72 h certificates renewed daily; timestamp required; identity re-validation | Online ID check, minutes to days | **Recommended** |
| Azure Artifact Signing, organization | Only if Sylin is a legal entity | Same | Same | "Sylin" | Same | Same | 1-20 business days | Viable if the entity exists |
| SignPath Foundation | Unlikely (visibility bar) | Free | Yes (public CA) | "SignPath Foundation" | Official connector; **manual approval per release** | Key held by SignPath; volume limits | Weeks to months | Later, not now |
| Certum Open Source | Probably (individuals; US applicants not confirmed) | EUR 49 cloud, or EUR 69 card then EUR 25 renewals | Yes | "Open Source Developer, <name>" | No official headless CI; scripted 2FA | 5,000 signatures/mo; 459-day cap; revoked if used commercially | 2-5 days | Budget fallback |
| SSL.com IV (or OV) + eSigner | Yes (IV); OV needs an organization | about $309 | Yes | Owner's name (IV) or Sylin (OV) | Official action; 2FA secret in GitHub; none | 20 signatures/mo on Tier 1 (Ghostlight needs about 12); 458-day reissue | 3-5 days | **Fallback** |
| Reseller OV (Sectigo, DigiCert, GlobalSign) | Mostly organizations | about $220 to $439 | Yes | Organization | USB tokens don't work on hosted runners; cloud costs more | 460-day cap | Days | Not recommended |
| EV | Organizations (SSL.com sells a sole-proprietor EV) | $400 and up | Yes | Organization | As OV | 460 days | Longer | Not recommended: no SmartScreen advantage since 2024 |
| Microsoft Store (MSIX) | Yes (free individual account) | Free | Store re-signs | Store name | Store certification per release | Store installs only | Store review | Not a fix; maybe a later channel |
| Sigstore / cosign | n/a | Free | **No** | n/a | Already used for attestations | n/a | n/a | Not viable for Windows trust |
| Foundations (Apache, OpenJS, LF, NumFOCUS, OpenSSF) | Hosted projects only | n/a | Yes | Foundation | Varies | Varies | Joining | Not viable now |
| Unsigned + false-positive submissions | Yes | Free | None | "Unknown publisher" | Manual per release | Smart App Control blocks it | Per case | Stopgap only |

## Per-candidate findings

### Azure Artifact Signing

- Generally available since January 2026. Organizations in the US, Canada, EU, UK and a few others;
  individuals only in the US or Canada (quickstart dated 2026-05-21, updated 2026-10-08). For an
  individual, the Azure billing account must be of type Individual, with a legal name and address
  that match a government ID. The certificate shows the name plus city, state and country; no
  custom CN or O (FAQ, 2026-05-14). A 2025 preview rule asked organizations for 3+ years of
  history; one Hacker News report says a younger business was verified after GA (unverified).
- $9.99/mo Basic (5,000 signatures, one profile of each type); $99.99/mo Premium. Billing starts at
  account creation and is not pro-rated; free, trial and sponsored subscriptions are refused.
- Certificates live 72 hours with keys in FIPS 140-3 Level 3 HSMs; timestamps from
  `http://timestamp.acs.microsoft.com` keep signatures valid. **Note:** the certificate-management
  page (2026-09-30) says these CAs are scheduled for removal from CCADB and are no longer in the
  Trusted Root Program, but remain on the Windows Platform Trust List. The Smart App Control page
  still speaks of Trusted Root Program CAs; Castle Game Engine reported in July 2026 that its
  Azure-signed builds pass Smart App Control. Test this before relying on it.
- SmartScreen reputation builds over time on the identity. Azure/artifact-signing-action issue 128
  records SmartScreen warnings from March to October 2026, which users tie to rotating intermediate
  CAs; Microsoft's answers changed over time.
- `azure/artifact-signing-action@v2`: Windows only; signs exe, dll and msix; OIDC through
  `azure/login`; the identity needs the Certificate Profile Signer role.
- Risks: Basic support can't open technical tickets; one developer reports a renewal failing
  verification with no human to reach, after which they moved to SignPath. Identity validations
  expire (renewal opens 60 days before); an expired validation stops signing.

### SignPath Foundation

- Terms (signpath.org/terms, marked "Draft", read 2026-10-23): OSI license for every component, no
  commercial dual-licensing, no proprietary parts (Ghostlight passes since ADR-0140); MFA for every
  member and named Author, Reviewer and Approver roles; a "Code signing policy" page on the home and
  download pages; manual approval of every signing request; builds verifiable from source; the
  publisher is "SignPath Foundation", which may revoke "immediately or retroactively" and accepts at
  its discretion; a "certain verifiable reputation" for downloadable executables; no hacking tools;
  a privacy policy for any data transfer; warnings before system changes; an uninstaller.
- 2026 practice: low-visibility declines self-reported by dictus-desktop (after about 7 weeks),
  EasyTab, Hibiki, Docent, Skrog and Kovrel. worktrunk (Rust) was approved after 13 days but still
  had no production certificate three months later. Castle Game Engine rejected it over per-release
  approvals, volume limits and the rule against signing bundled third-party tools. Legacy Update
  avoided the shared certificate so other projects' malware couldn't hurt its reputation.
- `signpath/github-action-submit-signing-request@v3`; every job leading to the request must run on
  GitHub-hosted runners; it waits 600 s by default, which a manual approval can exceed.
- Repo note: `ghostlight.exe` carries ProductVersion 5.0.0 (the relay revision) while the service
  and desktop carry 2.0.1; SignPath enforces one product version per build.

### Certum Open Source Code Signing

- EUR 49 cloud (SimplySign) or EUR 69 card and reader, then EUR 25 renewal codes; 5,000 signatures
  a month; issued only to individuals as "Open Source Developer, <name>"; revoked if it signs
  commercially distributed software; needs a public project page showing the applicant's role, an
  ID and a utility bill; 459-day cap from 2026-02-27. The cloud product page showed "out of stock"
  on 2026-10-23. US eligibility isn't stated.
- SimplySign has no command-line login; each session needs a 2FA code and lasts 2 hours. Projects
  script it from the seed behind the phone app's QR code (FrodeHus/elevate PR 165,
  jay0lee/certum-cloud-code-sign, azeitler/ssign), which makes that seed as sensitive as the key;
  three wrong codes lock the account. The card can't be used on hosted runners.

### SSL.com IV / OV with eSigner

- eSigner Tier 1: $20/mo or $180/yr for 20 signatures, $1 per extra, 30-day trial (page updated
  2026-03-25). IV certificate listed at $129 for a year (seen in search results; confirm). Personal
  name, no business documents, 3-5 days; 458-day cap since 2026-02-27. `SSLcom/esigner-codesign`
  keeps username, password, credential ID and the 2FA secret in GitHub secrets. draw.io Desktop
  used it until 2026-05-03, then moved to Azure.

### Reseller OV and EV

- About $220/yr Sectigo (USB token) to $400-439/yr DigiCert with cloud KeyLocker. Since June 2023
  the CA/Browser Forum requires keys on certified hardware or a cloud HSM; the 460-day cap took
  effect 2026-03-01 (CSC-31). Microsoft says EV no longer bypasses SmartScreen. More cost than Azure
  for the same behavior.

### Microsoft Store / MSIX

- Free individual registration since 2025-09-10; the Store re-signs MSIX and WinGet installs with
  `--source msstore`. It covers only Store installs, and MSIX virtualizes registry and AppData
  writes, which collides with native-host registration for Chrome, Edge and Brave and with clients
  that start the exe by path. Not a fix; a packaging trial first if ever a channel.

### Sigstore, foundations, status quo

- Windows trusts Authenticode chains to its code-signing roots; Sigstore's Fulcio isn't one, and its
  signatures sit outside the file. Keep the GitHub attestations for provenance.
- Foundation signing serves only hosted projects (Apache via DigiCert, OpenJS for Node.js); fiscal
  hosts only pay for purchases.
- Unsigned plus WDSI submissions: Microsoft takes no allow-list requests from developers; WinGet
  doesn't require signing for portable zips, but a Defender verdict blocks the PR until cleared;
  Smart App Control blocks unsigned files; SmartScreen reputation restarts every release.

## What developers report (paraphrased)

- Rick Strahl (July 2025): Trusted Signing took most of a workday to set up, signs slowly, but at
  about $120/yr is the least bad option; his old certificate rose from $180 for three years to
  nearly $1,000.
- ImageMagick (2023): the hardware-key rule broke CI signing and DigiCert wanted $629/yr; it later
  moved to Trusted Signing. SFTPGo (2024) moved for the same reason.
- draw.io Desktop (2026): moved from SSL.com to Azure, warning SmartScreen may flag a new identity
  for weeks.
- Castle Game Engine (2026-07): chose Azure, rejected Certum and SignPath; Smart App Control no
  longer blocks it.
- Legacy Update (2026-03): stays on Certum, distrusts a shared SignPath certificate.
- FrodeHus/elevate (2026-09): Certum Open Source with scripted 2FA.
- dictus-desktop and several others (2026): SignPath asked for public references, then declined.
- worktrunk (2026): unsigned builds flagged `Wacatac.B!ml`; SignPath approved but no certificate
  after three months.
- Hacker News: Azure is easy once running but onboarding is error-prone and support is paywalled;
  eligibility changed several times in a year; central signing services are a single point of
  failure; DB Browser for SQLite has used SignPath for over a year.
- gabrielmoraru.com (2026-09): an OV certificate still triggered SmartScreen until reputation built
  over weeks; EV is a waste for a small vendor.
- Eric Lawrence (2026): sign every file with a Trusted Root Program chain, send false positives to
  WDSI; Smart App Control can now be turned back on in Windows 11 25H2.
- Reddit could not be fetched and returned no threads, so it is not covered.

## Next steps

Owner:

1. Record the decision as an ADR or amendment (the four documents above say there is no
   certificate).
2. Choose the publisher name: your legal name (individual validation, fast) or "Sylin" (only as a
   legal entity, 1-20 business days).
3. Azure: a pay-as-you-go subscription with an Individual billing account matching your ID;
   register `Microsoft.CodeSigning`; take the Identity Verifier role; create the Basic account only
   when ready to validate; complete validation; create a Public Trust profile; create an Entra app
   with a federated credential for `sylin-org/ghostlight`, limited to a protected environment, with
   the Certificate Profile Signer role.
4. Keep WDSI d5b9b3cd moving and ask WinGet for a re-run when it clears; submit each future zip and
   its three exes before its WinGet PR.

Repo, once the owner decides:

1. `release.yml`, Windows leg of `release-binaries`: `id-token: write` and an optional protected
   environment; right after `cargo build`, `azure/login` (OIDC) and
   `azure/artifact-signing-action@v2` signing the three exes with an RFC 3161 timestamp and SHA256;
   then `Get-AuthenticodeSignature` must report Valid with the expected signer. Everything built
   later then covers the signed bytes.
2. `scripts/check-release-candidate.ps1`: check that the Windows exes are validly signed by the
   expected subject.
3. Update the four documents above and `docs/trust/supply-chain.md` in the same change (keep claims
   and code in lockstep).
4. Test a signed build on a Windows 11 25H2 VM with Smart App Control enforcing, including Chrome
   starting the native host.

## Sources

[s] marks a source seen only in search-result summaries.

- Microsoft: learn.microsoft.com/en-us/azure/artifact-signing/faq;
  .../artifact-signing/quickstart; .../artifact-signing/concept-certificate-management;
  azure.microsoft.com/en-us/pricing/details/artifact-signing/;
  learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options;
  .../package-and-deploy/smartscreen-reputation; .../develop/smart-app-control/overview;
  learn.microsoft.com/en-us/defender-xdr/developer-faq; github.com/Azure/artifact-signing-action
  and its issue 128; mc.merill.net/message/MC1458947; blogs.windows.com (free Store registration,
  2025-09-10) [s]; WinGet install docs [s]; MSIX flexible virtualization [s].
- SignPath: signpath.org; signpath.org/terms; docs.signpath.io/trusted-build-systems/github.
- Certum: shop.certum.eu/code-signing.html; .../open-source-code-signing-on-simplysign.html;
  .../open-source-code-signing.html; support.certum.eu/en/code-signing-required-documents/.
- SSL.com: ssl.com/guide/esigner-pricing-for-code-signing/; ssl.com/certificates/code-signing/;
  IV product page [s]; github.com/SSLcom/esigner-codesign [s].
- Other official: CA/B Forum ballot CSC-31 [s]; infra.apache.org/digicert-use.html [s]; DigiCert
  KeyLocker licensing [s].
- Reporting and blogs: devclass.com (2026-01-14); 404media.co (VeraCrypt account) [s];
  weblog.west-wind.com (2025-07-20); mjtsai.com (2023-10-30) [s]; advancedinstaller.com
  (ImageMagick) [s]; castle-engine.io codesigning; msys2.org/dev/code-signing/;
  gabrielmoraru.com; piers.rocks (2025-10-30); textslashplain.com (2026-01-27, 2026-04-28);
  codenote.net (2026); opencollective.com/opensak; sqlitebrowser.org/blog/signing-windows/;
  worktrunk.dev/code-signing/.
- GitHub: drakkan/sftpgo 1778 [s]; jgraph/drawio-desktop discussion 2415 [s]; LegacyUpdate
  discussion 442; FrodeHus/elevate PR 165; jay0lee/certum-cloud-code-sign [s]; azeitler/ssign [s];
  getdictus/dictus-desktop 38; wslkit/skrog 358; AdamSellin/EasyTab PR 64 [s]; PhyberApex/hibiki
  PR 605 [s]; max-sixty/worktrunk 3355; anomalyco/opencode 7592; basmulder03/repo-keeper 50;
  sigstore/fulcio 250 [s].
- Hacker News items 46594304, 47693565, 47696386, 47694045, 45856386, 45020917, 46133909.
