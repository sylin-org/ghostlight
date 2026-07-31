# Ghostlight Master Software Agreement (Template)

> **DRAFT -- template pending counsel review. Not an offer.** This document is published for
> transparency and early review. It becomes binding only when executed in writing by both
> parties after legal review.

This template sets out the terms on which Ghostlight software is licensed to a customer. It is
written in plain language and is meant to be read before it is negotiated. Bracketed, uppercase
items are placeholders to be settled during review.

## Relationship to standard forms

This template is shaped against the Common Paper Cloud Service Agreement v2.1, a
counsel-drafted standard form published under CC BY 4.0, adapted for Ghostlight's local-only
architecture. Reviewers who know that form will find the same skeleton here, with three
deliberate divergences: termination never disables the software (the Continuity Promise
replaces the standard access-termination clause); there is no vendor-side privacy and security
section, because no customer data flows to the vendor (see [dpa.md](dpa.md) and
[data-flows.md](data-flows.md)); and the license grant reflects the open-core split rather
than a single subscription license.

## 1. Definitions

"Vendor" means `[VENDOR LEGAL ENTITY AND FORM -- TO BE COMPLETED IN REVIEW]`, the provider
of the Ghostlight software. "Customer" means the organization
identified in the executed order. "Engine" means the Ghostlight automation engine, licensed
Apache-2.0 OR MIT. "Governance Module" means the Ghostlight governance components licensed under
the Ghostlight Commercial License (LICENSE-GOVERNANCE). "Software" means the Engine and the
Governance Module together. "Documentation" means the materials published in this repository,
including the trust center. "Order" means the ordering document that references this Agreement.

## 2. License grant

The Engine is licensed under Apache-2.0 OR MIT, and nothing in this Agreement narrows the
rights those licenses grant. The Governance Module is licensed under the Ghostlight Commercial
License (LICENSE-GOVERNANCE) for the term and scope stated in the Order. The Governance Module
is source-available: Customer may read and audit its source. The license is granted for
Customer's internal use in accordance with the Order; use restrictions on the Governance Module
are those stated in LICENSE-GOVERNANCE, and this Agreement adds none beyond them.

## 3. Deployment and operation

The Software runs entirely on Customer infrastructure. Vendor operates no service in the path
of Customer's use and receives no Customer data through the Software, as described in
[data-flows.md](data-flows.md).

## 4. Fees and taxes

Fees, billing period, and any applicable seat or licensee counts are stated in the Order.
Amounts and payment terms are `[TO BE COMPLETED IN REVIEW]`. Fees are exclusive of taxes;
Customer is responsible for applicable sales, use, and value-added taxes, excluding taxes on
Vendor's income. Seat and licensee figures are contractual terms only and are never enforced by
the Software at runtime.

## 5. Support

Vendor provides support as described in [support-policy.md](support-policy.md), including the
acknowledgment commitments and scope stated there. The specific commitments applicable to
Customer follow Customer's tier as identified in the Order.

## 6. Continuity

The Continuity Promise applies to this Agreement as described in [continuity.md](continuity.md).
The Software continues to operate regardless of license state, Vendor availability, or Vendor's
continued existence.

## 7. Intellectual property

As between the parties, Vendor retains all rights in the Software except for the licenses
expressly granted. Customer retains all rights in Customer's own data and configurations.

## 8. Confidentiality

Each party protects the other's non-public information disclosed under this Agreement using at
least reasonable care, and uses it only to perform under this Agreement. This section does not
apply to information that is public or independently developed.

## 9. Warranties

Each party represents that it has the full power and authority to enter into this Agreement.
Vendor warrants that it has the right to grant the licenses in this Agreement, and that
releases published during a paid term will not materially reduce the general functionality of
the Software. The Ghostlight binary has no remote-control channel: deployed binaries cannot
be degraded, disabled, or altered by Vendor at all, so the functionality warranty concerns
only what future releases offer, never what Customer already runs. The browser extension,
when installed from the Chrome Web Store, follows Chrome's store update mechanism; Customers
requiring change control over extension versions can self-host the extension and pin
versions through Chromium enterprise policy.

## 10. Disclaimer

Except as expressly stated in Section 9, Vendor disclaims all other warranties, whether express
or implied, including implied warranties of merchantability and fitness for a particular
purpose, and the Software is otherwise provided "as is" to the fullest extent permitted by law.

## 11. Limitation of liability

Neither party is liable for indirect, incidental, or consequential damages. Each party's
aggregate liability under this Agreement is capped at the General Cap Amount of
`[TO BE COMPLETED IN REVIEW]` (the standard form's default is the fees paid or payable in the
12 months before the claim). Claims arising from `[TO BE COMPLETED IN REVIEW]` are instead
subject to an Increased Cap Amount of `[TO BE COMPLETED IN REVIEW]`. The caps do not apply to a
party's indemnification obligations under Section 12, to breach of Section 8
(Confidentiality), or to `[TO BE COMPLETED IN REVIEW]`.

## 12. Indemnification

Vendor will defend and indemnify Customer against third-party claims alleging that the
Software, as provided by Vendor and used in accordance with this Agreement, infringes or
misappropriates that third party's intellectual property rights. If such a claim is upheld or
appears likely, Vendor may procure the right for Customer to continue use, modify the Software
to be non-infringing, or refund the prepaid fees for the remaining term; consistent with the
Continuity Promise, none of these remedies disables what Customer already runs. Customer will
defend and indemnify Vendor against third-party claims arising from Customer's own data,
configurations, or use of the Software in violation of this Agreement or applicable law.
Exclusions and procedures are `[TO BE COMPLETED IN REVIEW]`.

## 13. Term and termination

This Agreement runs for the term stated in the Order. Either party may terminate for material
breach not cured within 30 days of written notice, or immediately for a breach incapable of
cure. Termination ends the Governance Module's commercial license for future periods; it
does not disable, degrade, or interrupt the Software already deployed, consistent with the
Continuity Promise and ADR-0028.

## 14. Publicity

Neither party may use the other party's name or logo publicly without prior written consent.
Any reference or case-study commitment (for example, under a founding-program agreement) is
stated in the Order, not implied by this Agreement.

## 15. Compliance with laws

Each party will comply with applicable anti-bribery and anti-corruption laws in connection
with this Agreement. Customer will comply with applicable export control and sanctions laws in
its use and distribution of the Software, which contains cryptographic functionality.

## 16. Governing law and disputes

This Agreement is governed by the laws of `[TO BE COMPLETED IN REVIEW]`, and the parties submit
to the venue stated in `[TO BE COMPLETED IN REVIEW]`.

## 17. Notices

Notices to Vendor are sent to support@sylin.org. Notices to Customer are sent to the contact in
the Order.

## 18. General

This Agreement, together with the Order, is the entire agreement between the parties on its
subject matter. If any provision is unenforceable, the rest remains in effect. Neither party
may assign this Agreement without the other's consent, except in connection with a merger or
sale of substantially all assets.

Last reviewed: 2026-07-10 against v0.7.1 | Contact: support@sylin.org
