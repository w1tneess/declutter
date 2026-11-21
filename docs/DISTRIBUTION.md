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

**Decision (2026-10-21): SignPath Foundation.** The maintainer chose it: free for an MIT project, and it fits the
public GitHub Actions build this repository already has. Azure Artifact Signing stays as the fallback (end of the next
section). Do not buy an EV certificate for SmartScreen reasons. The Microsoft Store (Store-distributed apps are
re-signed by Microsoft and never shown SmartScreen warnings) is the only route with no reputation period at all, but it
means an MSIX package and a Partner Center account and is not planned for 1.0.

## Signing with SignPath Foundation

### The order problem: the first beta ships unsigned

SignPath Foundation's terms say "the project must already be released in the form that should be signed". There is no
Declutter 1.0 release yet, so the order has to be:

1. **Ship the first beta unsigned**, built by `release-v1.yml` from a tag while `SIGNING_PROVIDER` is unset (the signing
   jobs are skipped; the unsigned installer, portable zip and checksums are published). Put the README notice from
   "What the README should say in the meantime" next to the download. Smart App Control will refuse this build.
2. **Apply** to SignPath Foundation with that release as the evidence (checklist below).
3. **Configure** SignPath and GitHub once accepted (one-time setup below), set `SIGNING_PROVIDER` to `signpath`, and tag
   the next release: the first signed build.

### How the release workflow wires signing in

`.github/workflows/release-v1.yml` runs `build -> sign-app -> bundle -> sign-installer -> checksums -> publish`.
The app exe is signed **before** it is packed into the installer and the portable zip, then the installer is signed, so
the installed program and the installer both carry a signature (the zip's exe is the signed one; the zip itself is not
signed).

- `build` uploads the unsigned app exe as the workflow artifact `app-exe` and exposes that upload's id as the job
  output `app-exe-id`. `bundle` uploads `installer` (setup exe and portable zip, as before) and also the setup exe on
  its own as `installer-setup`, exposing that upload's id as `installer-setup-id`.
- `sign-app` and `sign-installer` call SignPath's connector `SignPath/github-action-submit-signing-request`, pinned to
  commit `f6d04783b4569d051e0c80105fe66e82819d0092` (the `v3` and `v3.0` tags; `v3` is a lightweight tag pointing
  straight at that commit, checked with the GitHub API on 2026-10-21). Inputs used, all read from the action's
  `action.yml` at that commit: `api-token`, `organization-id`, `project-slug`, `signing-policy-slug`,
  `github-artifact-id`, `wait-for-completion: true`, `wait-for-completion-timeout-in-seconds: 3600` (the default is
  600; the wait includes the manual approval in SignPath) and `output-artifact-directory`. `artifact-configuration-slug`
  is not set, so the project's default artifact configuration is used; `github-token` keeps its default, the job's
  `GITHUB_TOKEN`.
- The connector hands the artifact id to SignPath, which fetches the artifact, checks that it came from this
  repository's workflow on GitHub-hosted runners (its "trusted build system" check) and signs it; the signed result is
  extracted into `signed/`. `sign-app` re-uploads it as `app-exe` (overwrite), which `bundle` packs. `sign-installer`
  copies the signed setup exe over the unsigned one in the `installer` artifact and re-uploads that (overwrite), which
  `checksums` hashes and `publish` releases.
- Both jobs then check every exe with `Get-AuthenticodeSignature`: the status must be `Valid` and the signer
  certificate's subject must contain `CN=SignPath Foundation` (SignPath's terms: the certificate is issued to the
  Foundation, which is shown as the publisher).
- The signing jobs run only SHA-pinned actions and PowerShell checks: no checkout, no `npm`, no `cargo`, so no
  dependency or build script ever runs where the API token is reachable. Their permissions are `actions: read` (the
  connector reads the run and its artifact) and `contents: read`, nothing else.
- They run only on a real `v1.*` tag push and only when the repository **variable** `SIGNING_PROVIDER` is `signpath`.
  The workflow_dispatch dry run and a repository without the variable skip them and produce unsigned files. No dry run
  gets a write token; only `publish`, on a tag, does.
- They use the GitHub environment `release-signing`, the only place the API token exists.

### One-time setup (after acceptance)

GitHub (settings of `w1tneess/declutter`):

1. Environments: create `release-signing`. Required reviewers: the maintainer. Deployment branches and tags: selected
   only, with the tag rule `v1.*`.
2. In that environment add the **secret** `SIGNPATH_API_TOKEN` (an API token of a SignPath user with submitter
   permission on the signing policy) and the **variables** `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG` and
   `SIGNPATH_SIGNING_POLICY_SLUG`. Do not make the token a repository secret: every job could then read it.
3. Repository variable `SIGNING_PROVIDER` = `signpath`. Unsetting it switches signing off again.

SignPath (the organization SignPath Foundation sets up for the project):

1. Note the **organization id**, and create or confirm the **project** (its slug goes into `SIGNPATH_PROJECT_SLUG`).
2. **Trusted build system:** link the project to GitHub.com for the repository `w1tneess/declutter` (SignPath's GitHub
   trusted build system).
3. **Artifact configuration** (the project default) matching what the workflow uploads. `actions/upload-artifact`
   zips each artifact, so the root is a zip; each artifact holds exactly one exe at its root (`declutter.exe` in
   `app-exe`, `Declutter_<version>_x64-setup.exe` in `installer-setup`), so one configuration serves both requests:

   ```xml
   <artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
     <zip-file>
       <pe-file path="*.exe">
         <authenticode-sign/>
       </pe-file>
     </zip-file>
   </artifact-configuration>
   ```

   A wildcard must match exactly one file unless `max-matches` says otherwise, which is what is wanted here.
4. **Signing policy** for releases (its slug goes into `SIGNPATH_SIGNING_POLICY_SLUG`) with manual approval, the
   maintainer as **approver**, and the API token's user as **submitter**.
5. MFA on the maintainer's SignPath and GitHub accounts.

A tag build then pauses at each signing job twice: GitHub asks for the environment approval, then SignPath waits for
the signing request to be approved (up to an hour before the job gives up).

### Applying to SignPath Foundation: checklist

From SignPath Foundation's terms (signpath.org/terms; the apply page on signpath.org lists no procedure of its own):

- [ ] OSI-approved licence, without commercial dual licensing, for the project and every component: MIT for Declutter;
      the dependency licences are gated by `deny.toml`.
- [ ] Already released in the form to be signed: the unsigned first beta above.
- [ ] A download page that describes what the app does: the README and the GitHub release.
- [ ] A code signing policy on the project home page, linked with the words "Code signing policy", saying "Free code
      signing provided by SignPath.io, certificate by SignPath Foundation":
      [`CODE_SIGNING_POLICY.md`](CODE_SIGNING_POLICY.md), linked from the README.
- [ ] Team roles and members (authors, reviewers, approvers): in the policy, all held by the single maintainer.
- [ ] A privacy statement: in the policy (the app sends nothing; the installer may download Microsoft's WebView2
      bootstrapper).
- [ ] MFA for everyone with access, on GitHub and on SignPath.
- [ ] Built from its own source by a public CI build: `release-v1.yml` on GitHub-hosted runners.
- [ ] Consistent product name and version metadata in the signed files (see the version mismatch in `ROADMAP.md`).

### Fallback: Azure Artifact Signing

If SignPath Foundation declines the project, or its certificate turns out not to satisfy Smart App Control, Azure
Artifact Signing is the alternative (Basic $9.99 per month on a paid Azure subscription, identity validation,
individuals only in the US or Canada; Microsoft names it the preferred way to sign for Smart App Control). It would
replace the connector step in both signing jobs with `azure/login` (OIDC with a federated credential that trusts only
the `release-signing` environment, so `id-token: write` instead of `actions: read`) followed by
`Azure/artifact-signing-action` on the downloaded files; the job split and the rest of the pipeline stay as they are.

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

- **SignPath acceptance of this project.** Not applied for yet. Unknown: whether SignPath Foundation accepts a
  single-maintainer project in which one person is author, reviewer and approver (the terms speak of "team members"
  and do not say whether a team of one is enough), and whether an unsigned beta counts as "already released in the form
  that should be signed".
- **Smart App Control and SignPath Foundation certificates.** Microsoft says Smart App Control "only considers
  certificates issued by trusted providers" and names Azure Artifact Signing as "the preferred way"; I found no
  statement either way about SignPath Foundation's certificate. Test the first signed build on a machine with Smart App
  Control in enforcement mode before relying on it; if it is refused, the Azure fallback is the answer.
- **The exact certificate subject.** The workflow expects `CN=SignPath Foundation` in the signer subject, from the
  terms ("issued to SignPath Foundation"); the full subject string has not been seen on a real signed file.
- **Whether SignPath passes unlisted files in a zip through unchanged.** Its artifact configuration docs do not say,
  which is why `installer-setup` holds the setup exe alone instead of sending the whole `installer` artifact.
- **MFA.** The policy requires it; whether it is on for the maintainer's GitHub account was not checked here.
- **Azure eligibility** (fallback only). The maintainer's country was not checked against the list above
  (individuals: US or Canada only).
- **Reputation across certificate rotation** (fallback only). Azure certificates last three days; Microsoft's
  reputation text is about publisher identity and file hash, and I could not confirm in the documentation how
  reputation behaves across the rotating certificates.
- **The workflow's signing and attestation jobs have not run.** Signing needs an accepted SignPath project, and
  attestation needs a tag. The dry run covers build, bundle and checksums only; the first signed tag is the first
  test of the rest.
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
- Azure/artifact-signing-action (fallback; inputs were read from its `action.yml` at commit `c7ab2a8`, v2.0.0):
  https://github.com/Azure/artifact-signing-action
- SignPath Foundation terms (eligibility and conditions): https://signpath.org/terms
- SignPath GitHub trusted build system (artifact upload, GitHub-hosted runners, permissions):
  https://docs.signpath.io/trusted-build-systems/github
- SignPath action inputs (`action.yml` at commit `f6d04783b4569d051e0c80105fe66e82819d0092`, the `v3` tag):
  https://github.com/SignPath/github-action-submit-signing-request
- SignPath artifact configuration syntax (root element, zip root, a wildcard matches exactly one file by default):
  https://docs.signpath.io/artifact-configuration/syntax
- OV code signing: hardware key rule since 2023-06-01 and indicative prices (vendor pages, not independent):
  https://www.thesslstore.com/blog/code-signing-price-changes-as-cas-align-with-new-industry-standards/ and
  https://sectigo.com/signing-certificates/code-signing
