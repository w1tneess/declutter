# Distribution, trust and code signing

Owner: release. Facts below were checked against the sources listed at the end on 2026-10-21 (the dates of Microsoft's
pages are in the sources). Where something could not be confirmed it says so, in "What is not confirmed".

## Why signing is a requirement, not polish

On the maintainer's Windows 11 machine **Smart App Control is in enforcement mode and blocks unsigned executables that
have no reputation**. Every locally built Rust binary (build scripts, test executables, the app itself) fails to start
with os error 4551. Anyone with Smart App Control on who runs an unsigned Declutter gets the same result.

How Smart App Control decides (Microsoft's FAQ): it first asks a cloud service whether it can confidently predict the
app is safe; if it cannot, it allows the app only if it carries a valid signature from a trusted provider; unsigned
apps are blocked, and **there is no way to allow a single app**. The user's only options are to turn Smart App Control
off or to get a signed build. A new, unsigned, low-download binary has no cloud reputation, so a fresh Declutter 1.0
installer is exactly what gets blocked. The portable zip is no escape: the check applies to every executable, not only
downloaded ones.

SmartScreen is the milder cousin: an unsigned download shows "Windows protected your PC" and the user may choose
"Run anyway". Microsoft notes that on Windows 11 Smart App Control may supersede SmartScreen's application reputation,
and Smart App Control is the harder block above.

A 1.0 that cannot start on a default Windows 11 install is not shippable. Signing therefore gates the public 1.0
release, or the release must go out with the README notice at the bottom.

## Options

| | SignPath Foundation | Azure Artifact Signing (formerly Trusted Signing) | OV or EV certificate from a CA |
|---|---|---|---|
| Cost | Free for qualifying open source | Basic $9.99 per month (5,000 signatures, then $0.005 each); Premium $99.99 per month. Needs a paid Azure subscription (free, trial and sponsored ones are refused) | Roughly $200 to $340 per year on vendor pages (indicative; term length and token included vary) |
| Publisher shown | SignPath Foundation (the certificate is issued to the Foundation, so the Foundation is the publisher of the project) | Your validated legal name (individual or organization); no custom name | Your validated name |
| Who qualifies | OSI-approved licence on every component, no proprietary parts, actively maintained and already released in the form to be signed, functionality described on its download page, own source built verifiably, MFA for the team, author/reviewer/approver roles, a code signing policy on the project home page | Organizations in the US, Canada, EU, UK, Australia, New Zealand, Japan, South Korea, Singapore, Switzerland, Norway, Israel; individual developers only in the US or Canada. Identity validation takes 1 to 20 business days | Anyone who can pass the CA's validation |
| Key handling | Foundation holds the key (HSM); every release needs a manual approval | Microsoft holds the key in an HSM; certificates are valid three days and are renewed by the service, so timestamping is mandatory | Since 2023-06-01 the key must be on a FIPS-certified hardware token or cloud HSM, so signing from CI needs the CA's cloud signing product or a self-hosted runner with the token |
| CI fit | GitHub Actions connector, GitHub-hosted runners required | `Azure/artifact-signing-action`, OIDC login, no stored secret | Depends on the CA; usually extra cost for a cloud HSM service |
| Smart App Control | Not confirmed (see below) | Microsoft names it "the preferred way" to sign for Smart App Control | Accepted if issued by a trusted provider |
| EV | n/a | Issues no EV certificates and says it never will | EV no longer gives instant SmartScreen reputation (Microsoft), so it is not worth the premium for that purpose |

Recommendation: apply to **SignPath Foundation** first (free, and fits an MIT project), and set up **Azure Artifact
Signing** if the maintainer's location qualifies and a paid path is acceptable or SignPath says no. Do not buy an EV
certificate for SmartScreen reasons. The Microsoft Store (Store-distributed apps are re-signed by Microsoft and never
shown SmartScreen warnings) is the only route with no reputation period at all, but it means an MSIX package and a
Partner Center account and is not planned for 1.0.

## How the release workflow wires signing in

`.github/workflows/release-v1.yml` runs `build -> sign-app -> bundle -> sign-installer -> checksums -> publish`.
The app exe is signed **before** it is packed into the installer and the portable zip, then the installer is signed, so
the installed program and the installer both carry a signature (the zip's exe is the signed one; the zip itself is not
signed). The two signing jobs:

- run only SHA-pinned actions (download-artifact, azure/login, artifact-signing-action, upload-artifact): no `npm`, no
  `cargo`, so no dependency or build script ever runs where a signing identity is reachable;
- run only on a real `v1.*` tag push and only when the repository **variable** `SIGNING_PROVIDER` is `azure`; the
  workflow_dispatch dry run and a repository without the variable skip them (unsigned artifacts, which Smart App Control
  will refuse);
- use the GitHub environment `release-signing`, hold `id-token: write` and nothing else, and verify with
  `Get-AuthenticodeSignature` that the result is `Valid`;
- authenticate with OIDC. No client secret exists anywhere.

### One-time setup for Azure Artifact Signing

1. In Azure: register `Microsoft.CodeSigning`, create a Signing account, complete identity validation, create a
   **Public Trust** certificate profile.
2. Create an app registration with a **federated credential** for GitHub Actions with subject
   `repo:w1tneess/declutter:environment:release-signing`, and give it the role **Artifact Signing Certificate Profile
   Signer** on the signing account.
3. In GitHub: Settings, Environments, create `release-signing`; add required reviewers (the maintainer); under
   deployment branches and tags allow only the tag pattern `v1.*`. Add these environment **variables** (they are ids, not
   secrets): `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID`, `AZURE_SIGNING_ENDPOINT` (the regional
   `https://<region>.codesigning.azure.net` URL), `AZURE_SIGNING_ACCOUNT`, `AZURE_SIGNING_PROFILE`.
4. Set the repository variable `SIGNING_PROVIDER` to `azure`. Unsetting it switches signing off again.

A tag build then pauses at each signing job until the maintainer approves it in the Actions UI.

### If SignPath is chosen instead

SignPath's connector is also a pinned action (`SignPath/github-action-submit-signing-request`, v3 is commit
`f6d04783b4569d051e0c80105fe66e82819d0092`). Replace the three Azure steps in `sign-app` and `sign-installer` with it,
keeping the environment and the job split. What differs, from SignPath's documentation:

- The unsigned file must have been uploaded with `actions/upload-artifact` v4 or later in the same workflow run, and the
  step receives that upload's `artifact-id` as `github-artifact-id` (so `build` and `bundle` must expose the id as a job
  output). SignPath verifies that the artifact came from this repository's workflow on GitHub-hosted runners (its
  "trusted build system" check), which is the origin verification that makes a stolen token useless from elsewhere.
- The action needs an API token (`api-token` is a required input). It lives only in the `release-signing` environment,
  never in a build job.
- Needs `actions: read` and `contents: read` on the signing job, plus the organization id, project slug and signing
  policy slug as environment variables.
- Every release is approved in SignPath as well, and the artifact configuration must match what is uploaded (a zip by
  default).

### Applying to SignPath Foundation

SignPath's public pages list conditions but no step-by-step procedure (the application page returned no content to
me). What is known: apply on signpath.org, meet the eligibility conditions in the table, publish a **code signing
policy** on the project's home page (author, reviewer and approver roles; build system; what is signed), turn on MFA for
everyone with access, and keep product name and version metadata consistent. Prepare before applying: a released 1.0
in the form to be signed, a download page that describes the app, a README/website section for the policy, and a public
CI build (this repository's workflow is that build).

## SmartScreen reputation: what to expect

From Microsoft's documentation: SmartScreen looks at publisher reputation (is it signed, by whom) and file hash
reputation (has this file been downloaded without problems). A signed binary that is new can still be flagged until its
hash or its publisher accumulates a record; the prompt stops once the file has enough download history, which Microsoft
says can take several weeks and hundreds of clean installs from a wide audience, and cannot be sped up for consumer
machines. An unsigned file starts from zero with every version. A signed file lets reputation carry over to new versions
signed by the same identity, so keep one signing identity and sign every release. Never sign anything that behaves like
unwanted software, or the certificate itself earns a bad reputation.

Expect warnings on the first weeks of a signed 1.0 for some users. A valid signature, unlike no signature, is what lets
Smart App Control run the app at all.

## What the README should say in the meantime

Until a signed release exists, say it plainly near the download link:

> **Declutter 1.0 is not code-signed yet.** Windows may warn about it ("Windows protected your PC": choose More info,
> then Run anyway). If **Smart App Control** is on, Windows will refuse to start it and there is no per-app override;
> Declutter will not ask you to turn that protection off. Signed releases are planned. Until then you can check that
> your download is the one built by this repository's public workflow:
> `Get-FileHash .\Declutter_x.y.z_x64-setup.exe -Algorithm SHA256` against `SHA256SUMS.txt`, and
> `gh attestation verify .\Declutter_x.y.z_x64-setup.exe --repo w1tneess/declutter`.

(The attestation step runs only on a real tag; it has not yet run for this repository's v1 pipeline.) The v0.x Python
releases are unsigned too and fall under the same Smart App Control rule; the same notice applies to them.

## What is not confirmed

- **SignPath eligibility for this project.** Not applied for. Unknown: whether a single-maintainer project satisfies the
  author/reviewer/approver role requirement, and whether the Foundation accepts a project whose first release is the
  one to be signed.
- **Smart App Control and SignPath Foundation certificates.** Microsoft says Smart App Control "only considers
  certificates issued by trusted providers" and names Artifact Signing as preferred; I found no statement either way
  about SignPath Foundation's certificate. Test with a real signed build on a machine with Smart App Control in
  enforcement mode before relying on it.
- **Azure eligibility.** The maintainer's country was not checked against the list above (individuals: US or Canada only).
- **Reputation across certificate rotation.** Azure certificates last three days; Microsoft's reputation text is about
  publisher identity and file hash, and I could not confirm in the documentation how reputation behaves across the
  rotating certificates.
- **The workflow's signing and attestation jobs have not run.** Signing needs real accounts, and attestation needs a tag.
  The dry run covers build, bundle and checksums only; the first signed tag is the first test of the rest.
- **The Tauri build assumptions.** The workflow assumes `@tauri-apps/cli` in `ui/package.json` and `src-tauri/` beside
  `ui/` (`tauri build --no-bundle`, then `tauri bundle --bundles nsis`), and that the build works with
  `npm ci --ignore-scripts`. `@tauri-apps/cli` is now in `ui/package.json` and `src-tauri/` sits beside `ui/`, but the
  workflow itself has never run, so these steps (and `--ignore-scripts`, which CI's own `npm ci` does not use) are
  untested.
- Prices are those on the pages read on 2026-10-21; CA prices come from reseller pages and are indicative.

## Sources

- Smart App Control FAQ, decision process and "no per-app bypass":
  https://support.microsoft.com/en-us/windows/smart-app-control-frequently-asked-questions-285ea03d-fa88-4d56-882e-6698afdb7003
- Sign your app for Smart App Control (updated 2026-09):
  https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control
- SmartScreen reputation for Windows app developers (updated 2026-08): reputation signals, EV no longer bypasses, SAC
  supersedes SmartScreen reputation: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
- Artifact Signing quickstart (regions, individual identity validation, processing time, updated 2026-10):
  https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart
- Artifact Signing FAQ (paid subscription, no EV, no custom CN, SmartScreen, three-day certificates):
  https://learn.microsoft.com/en-us/azure/artifact-signing/faq
- Artifact Signing signing integrations (auth, role, timestamp): https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations
- Artifact Signing pricing (Basic $9.99 per month, Premium $99.99; via search result, the pricing page itself was not
  fetched): https://azure.microsoft.com/pricing/details/artifact-signing/
- Azure/artifact-signing-action (inputs read from its `action.yml` at the pinned commit):
  https://github.com/Azure/artifact-signing-action
- SignPath Foundation terms (eligibility and conditions): https://signpath.org/terms
- SignPath GitHub trusted build system (artifact upload, GitHub-hosted runners, permissions):
  https://docs.signpath.io/trusted-build-systems/github
- SignPath action inputs (`action.yml` at v2 and v3 tags): https://github.com/SignPath/github-action-submit-signing-request
- OV code signing: hardware key rule since 2023-06-01 and indicative prices (vendor pages, not independent):
  https://www.thesslstore.com/blog/code-signing-price-changes-as-cas-align-with-new-industry-standards/ and
  https://sectigo.com/signing-certificates/code-signing
