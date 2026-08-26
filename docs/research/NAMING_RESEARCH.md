# Naming Research — Final Product Name for "Dev Command Center" (codename)

Status: Round 1 COMPLETE — **all three Round 1 finalists REJECTED by Human
Naming Review (2026-08-26); see §R2.0.** Round 2 COMPLETE — see
"## Round 2 — Distinctive Brand Search" below, which supersedes Round 1 as
the active recommendation. This gate is RESEARCH + DOCUMENT ONLY: nothing has
been renamed, no repository/org/domain has been created, claimed, or
purchased, and no implementation exists. Legal/trademark clearance remains a
later human release gate (PRD §17; AGENTS.md rule 9).

Human architecture approval recorded this date:
`HUMAN_ARCHITECTURE_GATE=APPROVED` (see docs/product/STATUS.md).

---

## R2.0 — Round 1 outcome (recorded verbatim from Human Naming Review)

The previous three finalists are PERMANENTLY REJECTED and must not be
re-ranked, defended, or revived absent strong new evidence:

1. **Rollcab** — exact existing long-standing/current PETZL product name
   ROLLCAB; also widely used around physical roll-cab/tool-cabinet
   terminology; unacceptable search/brand distinctiveness even if software
   trademark collision appears low. *(This PETZL conflict was missed by the
   Round 1 screen — a screening gap now addressed by the Round 2 physical-
   product rule.)*
2. **Mooring** — active developer/software collisions exist now; active
   self-hosted PaaS/control-plane project named Mooring; active PyPI software
   named mooring; additional developer/Git/GitHub tooling uses the name.
   Direct developer-category collision.
3. **Workstead** — multiple active established businesses; active mobile
   software; active automation/software service; unacceptable brand/search
   collision.

Round 1 is preserved below unchanged as historical research.

---

## 1. Naming strategy

Product identity being named:

> A local-first, cross-platform developer workspace orchestrator that
> discovers existing launchers/tools, embeds PTY-backed terminal members,
> launches authorized external GUI applications, organizes local and remote
> tools into persistent workspaces, interoperates with ssh/tmux, runs on
> Linux + Windows, requires no cloud/account, and enforces a conservative
> discovery → review → authorization model.

Core proposition: **Your existing dev tools. One persistent workspace.**

Strategy chosen:

1. **Metaphor class: the craftsman's/workshop's place and equipment.** The
   product's essence is *organized permanence for tools* — not intelligence,
   not the cloud, not a shell. Workshop/marina/trade vocabulary carries
   exactly that connotation and has strong precedent in developer-tool
   branding without implying AI or terminal-only scope.
2. **One real English word preferred**, coined blends acceptable only when
   instantly parseable. Real words age better, are pronounceable
   internationally, and read well in a menu bar.
3. **Descriptive-generic roots are acceptable when the exact compound is
   rare** — rarity gives search distinctiveness even when the root word is
   common (contrast: naming a product "Monitor").
4. Hard filters applied before any screening: not AI-flavored, not cloud-
   flavored, not IDE/editor-sounding, not terminal-emulator-sounding, not a
   generic OS-launcher claim, ≤ ~2 short words, lowercase-friendly, CLI name
   ideally ≤ 12 characters, no awkward spellings, no trivial variations of
   the eleven already-rejected names (§9 constraints from tasking).
5. Domain optimizability was deliberately subordinated to name quality
   (.com desirable, not required, for OSS V1).

## 2. Collision-screen methodology

Applied to every serious candidate (defined below):

- **A. General web**: quoted exact-name search; name + software / developer /
  terminal / workspace / devtools. Performed via current public web search,
  retrieval date 2026-08-26.
- **B. GitHub**: organization/user namespace checks and repository searches
  (`https://api.github.com/search/repositories?q=<name>+in:name`, sorted by
  stars, plus targeted org lookups surfaced by web results).
- **C. Package ecosystems** (checked for every candidate): npm
  (`registry.npmjs.org/<name>`), crates.io (`crates.io/api/v1/crates/<name>`),
  PyPI (`pypi.org/pypi/<name>/json`). HTTP 404 = name unregistered at
  retrieval time; HTTP 200 followed by description/version inspection to
  judge whether the occupant is an active developer-facing project.
- **D. Product/company collisions**: software companies, SaaS, desktop apps,
  infrastructure products, open-source projects. A collision in an unrelated
  tiny/non-software context was recorded but not treated as fatal; a
  collision with a developer tool, software company/product, active package/
  library, desktop tool, or terminal/workspace product was treated as a
  strong rejection signal.

Screened counts: ≥ 40 candidates generated internally → semantic/fit filter
→ 19 serious candidates fully screened → 3 finalists deep-screened.

### Internal brainstorm pool (filtered out before screening, listed for honesty)

Muster*, Shipshape*, Wheelhouse*, Homeport*, Toolchest*, Slipway*, Mooring*,
Fairlead*, Capstan*, Binnacle*, Pilothouse*, Mansio*, Castra*, Flotilla*,
Statio*, Millwright*, Berth, Hangar, Halyard, Forestay, Cleat, Rollcab*,
Flybridge, Workstead*, Argosy, Bosun, Quarterdeck, Stevedore, Garrison,
Bothy, Coxswain, Chandler, Porter, Shipwright, Smithy, Anvil, Forge-family,
Cockpit, Mission Control, Waypoint, Keel, Rudder, Tiller, Helm, Drydock,
Panoply, Signalbox, Roundhouse, Homeroom, Caboose, Switchyard, Officina,
Stanza, Stave, Cairn, Gantry, Readyroom, Tarmac, Pilothouse variants.
(`*` = advanced to serious screening.) Pre-filter rejections were made on
known hard collisions (Smithy = AWS framework; Shipwright = CNCF project;
Porter = getporter.org; Keel = keel.sh; Rudder = rudder.io; Helm/Tiller =
Kubernetes ecosystem; Cockpit = cockpit-project.org; Waypoint = HashiCorp;
Anvil = multiple products; Forge-family = category saturation; Coxswain =
spelling; Chandler = pop-culture dominance; Drydock/Panoply/Signalbox/
Roundhouse/Bosun/Quarterdeck = dock-family pattern reuse or known software/
historic-software collisions) or weak fit (Cleat, Berth, Hangar, Argosy,
Garrison, Stevedore, Halyard, Flybridge, Cairn, Gantry, Readyroom, Tarmac,
Caboose, Homeroom, Switchyard, Officina, Stanza, Stave — generic-word SEO
problems, wrong connotations, or occupied packages). All names marked `*`
plus Mooring/Rollcab/Workstead/Bothy/Statio/Flotilla/Fairlead/Capstan/
Binnacle/Millwright received the full §2 screen below.

## 3. Serious candidates considered (all screened 2026-08-26)

| # | Candidate | npm | crates | PyPI | Verdict |
|---|---|---|---|---|---|
| 1 | muster | taken | taken | taken | REJECT |
| 2 | shipshape | taken | taken | taken | REJECT |
| 3 | wheelhouse | taken | free | taken | REJECT |
| 4 | homeport | taken | free | free | REJECT |
| 5 | toolchest | taken | taken | taken | REJECT |
| 6 | slipway | taken | taken | taken | REJECT |
| 7 | fairlead | free | free | free | REJECT |
| 8 | capstan | taken | taken | taken | REJECT |
| 9 | binnacle | taken | taken | taken | REJECT |
| 10 | pilothouse | taken | free | free | REJECT |
| 11 | mansio | free | free | free | REJECT |
| 12 | castra | free | free | free | REJECT |
| 13 | flotilla | taken | taken | taken | REJECT |
| 14 | millwright | taken | taken | taken | REJECT |
| 15 | forestay | free | free | free | REJECT |
| 16 | statio | taken | free | taken | REJECT |
| 17 | bothy | free | free | free | REJECT |
| 18 | **mooring** | taken (tiny lib) | **free** | taken (tiny tool) | FINALIST |
| 19 | **rollcab** | **free** | **free** | **free** | FINALIST |
| 20 | **workstead** | **free** | **free** | **free** | FINALIST |

(Counts include the three finalists; 17 serious candidates were rejected.)

## 4. Rejection reasons (serious candidates)

1. **Muster** — Active software field: Muster (muster.com, grassroots-
   advocacy SaaS); beamlabco/muster team-productivity **TUI/CLI** on GitHub;
   consumer apps (muster.io App Store / Google Play 2026); G2/GetApp-listed
   vendor. Multiple direct developer-tool signals.
2. **Shipshape** — google/shipshape static-analysis platform (archived but
   canonical); active PyPI `shipshape` (2026, naval-engineering library);
   shipshape.ai company; npm daemon/CLI for shipshape.io. Crowded dev space.
3. **Wheelhouse** — Wheelhouse (usewheelhouse.com): funded revenue-management
   **SaaS**, 51–100 staff, $16M raised; wheelhouse.software custom-software
   firm; wheelhouse.com PM tooling; stale-but-present npm `wheelhouse` CLI.
   Strong software-company collision field.
4. **Homeport** — github.com/**homeport** is an established Go dev-tools
   organization (dyff ≈1.9k★, termshot ≈1k★, havener) — i.e., an existing
   recognized *developer-tools* brand in exactly our space; fresh npm
   `homeport` v0.0.1 deploy tool (July 2026); USCG "Homeport" portal.
5. **Toolchest** — Toolchest Inc. (trytoolchest.com): Y Combinator-backed
   computational-biology cloud platform (active); crates.io/npm/PyPI
   packages; other `toolchest` libs. Funded software-company collision.
6. **Slipway** — Surprisingly crowded: sailscastshq/slipway = active
   self-hosted deployment platform with `slipway-cli` on npm (786 commits);
   slipwayhq GitHub org = Rust dashboard framework (slipway.co);
   getnelson/slipway CI binary; AGhost-7/slipway containerized-dev-env tool;
   `npx slipway` Claude Code setup tool; Slipways game.
7. **Fairlead** — fairlead.dev: active developer-facing ad-exchange
   infrastructure (Rust/Axum, SDKs); Fairlead Integrated (fairlead.com,
   large defense/maritime contractor); verified GitHub orgs `fairlead`,
   Fairlead-Software, Fairlead-Advisors. Crowded across software+industry.
8. **Capstan** — Live registered trademark CAPSTAN (Celemony Software GmbH,
   Nice classes 9 & 42 — audio-restoration software; Reg. #4144870);
   Capstan Therapeutics (acquired by AbbVie 2025); capstan.com manufacturer;
   npm `capstan` = VPS lifecycle CLI; PyPI `capstan` webhook inspector.
   Direct class 9/42 registration = potential conflict — legal review
   territory.
9. **Binnacle** — binnacle-app/Binnacle: "local-first native desktop
   workbench" for Cloudflare data services (April 2026) — a directly
   adjacent developer-desktop-workspace product; jeffbstewart/Binnacle
   OpenTelemetry log service; binnacle.io logging service + npm client;
   Traackr/binnacle Helm CLI; seayniclabs/binnacle. Crowded.
10. **Pilothouse** — PilotHouse (pilothouseapp.com, active booking SaaS +
    Hyperweb Media's ThePilothouse); Pilothouse Software (marine regulatory
    software company, LinkedIn); Pilothouse Consulting (SharePoint tech
    consultancy). Multiple software businesses.
11. **Mansio** — MANSIO GmbH (Aachen): funded (seven-figure seed, 2025)
    logistics **software** startup, German Startup Cup winner 2025, Bosch
    L.OS partner — notably using the identical Roman etymology; plus Mansio
    real-estate startup (mansio.com). Active software collisions.
12. **Castra** — Castra AB (Sweden): IT consulting group, ≈330 employees;
    castratech.com cybersecurity consultancy; CastraCS managed security;
    CB Insights-listed network-management software company "Castra".
    Multiple IT/software firms.
13. **Flotilla** — Flotilla IoT (fleet-management SaaS, active);
    flotilla.app Taiwan software company (Crunchbase-listed); Flotilla Group
    UK ESG platform; Flotilla Technologies (Kochi) dev shop. Multiple active
    software brands.
14. **Millwright** — npm `millwright` = "build tool"; Northwood-Systems/
    millwright = self-hosted LLM router in Rust (July 2026, active);
    PyPI/crates wrappers; profession-term dominance buries discoverability.
    Two developer-tool collisions.
15. **Forestay** — Forestay Capital (Geneva/London): enterprise-AI/SaaS VC
    that states "Forestay® is a **registered trademark in the USA**".
    Registered mark in the technology-investment space = material conflict.
16. **Statio** — vsanthanam/Statio iOS system monitor; Statio warehouse-
    management app (App Store 2026); statio-app (BPS Bontang); noorapps
    Statio Android app; PyPI statistics lib. Several small-but-active apps;
    weak distinctiveness vs "station".
17. **Bothy** — deakdotdev/bothy = desktop app "a place for solo developers'
    tasks, notes, time, Git activity"; sp00nznet/bothy = multi-provider
    infrastructure deployment console (web + Windows client + CLI);
    Bothy Technology GitHub org; grant-management software docs repo.
    Two active developer desktop/console products.

## 5. Finalist comparison table

Deep-screened finalists (retrieval 2026-08-26):

| Dimension | **Rollcab** | **Mooring** | **Workstead** |
|---|---|---|---|
| Meaning | Mechanic's rolling tool cabinet: every tool has its drawer; rolls to wherever you work | Permanent tie-up where your vessel(s) are secured — persistence embodied | "Stead" = standing place; a standing place for work |
| Fit to "Your existing dev tools. One persistent workspace." | Strong (organization + portability of your own tools) | Strong (permanence/securing of your own vessels/tools) | Strong (literal "work-place") |
| Software collisions found | None | None significant (generic marine-industry term usage: Online Mooring LLC marina SaaS, DNV mooring-analysis modules — different markets, generic-descriptive use) | Workstead design studio (Brooklyn/Hudson, est. 2009, AD100, owns workstead.com) **and** Workstead HR/payroll software (worksteadhr.com) |
| Package registries | Free: npm, crates.io, PyPI | crates free; npm = small Node hook lib; PyPI = small notebook-sharing tool (both obscure, non-workspace) | Free: npm, crates.io, PyPI |
| GitHub | `rollcab` user holds only zero-star toy repos (clock/journal) — effectively dormant | personal user `mooring` with small utilities (≤35★); no product | negligible |
| Trademark preliminary screen | No ROLLCAB marks surfaced; root is a descriptive trade term | No conflicting software marks surfaced; strongly generic/descriptive nautical term | Design-studio brand strength = elevated opposition risk; plus same-name software product exists |
| Domains (RDAP, retrieval-day) | rollcab.dev **free**, rollcab.app **free** (.com registered/parked) | .com/.dev/.app all registered | .com/.dev/.app all registered |
| Pronunciation (international) | ROLL-cab — trivially parseable | MOOR-ing — simple; minor vowel variation in some languages | WORK-sted — simple; "stead" slightly archaic |
| Risk notes | Informal/trade register; toolbox-industry term ownership of generic search results | Marine industry dominates raw search | Two established brands share the name |

## 6. Preliminary trademark screen — results and limitations

Method: public web searches including USPTO/WIPO/EUIPO surfaces and public
aggregators (Trademarkia pages surfacing USPTO records) for each finalist and
each rejected candidate flagged above.

Findings (preliminary, NOT legal clearance):

- **ROLLCAB**: no material conflict found in the preliminary public screen.
  No live or dead federal marks for "ROLLCAB" surfaced; usage found is
  descriptive product-category language in the tool-storage industry.
- **MOORING**: no material conflict found in the preliminary public screen
  for software classes. The word is highly descriptive/generic in the
  maritime domain (descriptive marks receive narrower protection, which cuts
  both ways). One marina-management vendor trades as "Online Mooring"
  (different market, composite name).
- **WORKSTEAD**: potential conflict — legal review required. A prominent,
  long-established design studio operates under the exact name and owns
  workstead.com, and a same-name HR/payroll software product also exists.
- **Capstan** (rejected candidate): live USPTO registration in classes 9 and
  42 (Celemony) — recorded as evidence for its rejection.
- **Forestay** (rejected candidate): owner publicly asserts a USA registered
  trademark — recorded as evidence for its rejection.

Limitations (stated plainly):

- The interactive USPTO Trademark Search system, WIPO Global Brand Database,
  and EUIPO eSearch+/TMview are JavaScript applications or credentialed APIs
  that could not be reliably queried programmatically in this pass; Justia's
  aggregator returned automated-access denials. Findings therefore derive
  from indexed public records surfaced by general web search plus registry
  API spot-checks, and MUST be treated as indicative, not exhaustive.
- **No statement here constitutes "trademark cleared".** Formal clearance in
  relevant Nice classes (9, 42, and advisably 35/38/45 neighbors) by
  qualified counsel remains a mandatory later release gate per PRD §17 and
  the public-release gates in AGENTS.md rule 9.

## 7. Domain / repository / package signals (finalists)

Signals gathered 2026-08-26 via RDAP (rdap.org; HTTP 200 = registered,
404 = unregistered at retrieval time), GitHub API, and package registries.
Availability is point-in-time; nothing was registered or reserved.

| Signal | Rollcab | Mooring | Workstead |
|---|---|---|---|
| .com | registered (third party) | registered | registered (design studio) |
| .dev | **unregistered** | registered | registered |
| .app | **unregistered** | registered | registered |
| GitHub name-space | `rollcab` account dormant/toys; clean path via new org e.g. `rollcab-app` | `mooring` personal account, small utilities | negligible presence |
| npm | **free** | small hook lib | **free** |
| crates.io | **free** | **free** | **free** |
| PyPI | **free** | small tool | **free** |
| Repo-slug suggestion | `rollcab-app/rollcab` | `mooring-dev/mooring` | `workstead-app/workstead` |

Per strategy §1.5, domain availability did not override name quality — but
among finalists Rollcab is also the strongest positionally.

## 8. Scoring

Rubric (as specified in the gate instructions; unchanged):
semantic fit 25 · collision risk 25 · memorability/pronunciation 15 ·
developer credibility 10 · search distinctiveness 10 · CLI/repository
usability 10 · brand extensibility 5.

| Criterion (max) | Rollcab | Mooring | Workstead |
|---|---|---|---|
| Product semantic fit (25) | 21 | 23 | 22 |
| Collision risk (25) | 24 | 20 | 14 |
| Memorability/pronunciation (15) | 13 | 12 | 13 |
| Developer credibility (10) | 8 | 7 | 8 |
| Search distinctiveness (10) | 8 | 6 | 4 |
| CLI/repository usability (10) | 10 | 8 | 8 |
| Brand extensibility (5) | 4 | 4 | 4 |
| **Total (/100)** | **88** | **80** | **73** |

Scoring rationale highlights: Rollcab wins on the two highest-weighted
practical dimensions combined — a completely clean software collision field
(free on all three registries, dormant GitHub handle, no marks surfaced,
.dev/.app open) and frictionless CLI/repository identity (`rollcab`, seven
letters, unambiguous). Its semantic fit (21) trails Mooring (23) because
"mooring" expresses *persistence* more literally, but Mooring loses points
on collision risk (occupied npm/PyPI names, taken domains, marine-industry
search dominance) and search distinctiveness. Workstead's excellent literal
meaning cannot offset sharing a name with a famous design studio and a
same-name software product.

## 9. Recommendation

### PRIMARY — **Rollcab**

The mechanic's roll cab is the exact physical analogue of the product: a
single, permanent, organized home where every tool has its place, wheeled to
wherever the work happens. It is one word, seven letters, trivially
pronounceable, unmistakably workshop-not-AI/not-cloud/not-terminal-only, and
it survived a full collision sweep with zero software conflicts, zero
surfaced trademark conflicts, and free npm/crates.io/PyPI names. It reads
well as a display name ("Rollcab"), a window title, and a binary
(`rollcab`), and the drawer/cabinet vocabulary extends naturally to product
concepts (workspaces ↔ drawers).

Decision: adopt **Rollcab** as the proposed final name, subject to formal
legal clearance and the human naming ratification gate.

### SECONDARY fallback — **Mooring**

If human review rejects Rollcab, **Mooring** is the evidenced runner-up: the
strongest pure expression of "persistent", clean in developer software
beyond two tiny unrelated packages, with no surfaced trademark conflicts.
Trade-offs to accept: all three key domains are taken (an OSS V1 can launch
without them), and raw-word search returns marine content first.

## 10. Proposed canonical forms (NOT applied anywhere yet — no rename performed)

### Rollcab (PRIMARY)

- Product display name: **Rollcab**
- Repository slug: `rollcab` (under a new organization, suggested `rollcab-app`)
- Binary/CLI name: `rollcab`
- Tauri identifier direction: `dev.rollcab.desktop` (or equivalent
  reverse-DNS under whichever domain is secured; direction only — no domain
  registered)
- Config-directory naming direction: Linux `$XDG_CONFIG_HOME/rollcab` +
  `$XDG_STATE_HOME/rollcab`; Windows `%APPDATA%\rollcab` (resolved via Known
  Folder APIs per ARCHITECTURE §12)

### Mooring (SECONDARY fallback)

- Product display name: **Mooring**
- Repository slug: `mooring` (under a new organization, suggested `mooring-dev`)
- Binary/CLI name: `mooring`
- Tauri identifier direction: `dev.mooring.desktop` (direction only; domain
  strategy unresolved because mooring.dev/.com are third-party registered)
- Config-directory naming direction: `$XDG_CONFIG_HOME/mooring` /
  `%APPDATA%\mooring`

### Workstead (screened finalist, not recommended)

- Product display name: Workstead
- Repository slug: `workstead`
- Binary/CLI name: `workstead`
- Tauri identifier direction: `dev.workstead.desktop` (direction only)
- Config-directory naming direction: `$XDG_CONFIG_HOME/workstead` /
  `%APPDATA%\workstead`

## 11. Gate outcome summary

- 19 serious candidates screened with primary-source evidence (registries,
  GitHub API, RDAP, current web).
- 17 rejected with recorded reasons; 0 contradictions found against PRD §17
  criteria after scoring.
- Preliminary public trademark screen completed for all finalists with
  stated limitations; nothing is claimed as legally cleared.
- No rename, repository creation, domain registration, commit, push, or
  dependency action performed.

Round 1 PRIMARY (SUPERSEDED — rejected by Human Naming Review): Rollcab
Round 1 SECONDARY (SUPERSEDED — rejected by Human Naming Review): Mooring

~~NAMING_GATE=READY_FOR_HUMAN_REVIEW~~ *(superseded by Round 2 below)*

---

# Round 2 — Distinctive Brand Search

Status: COMPLETE, retrieval date 2026-08-26. Research + documentation only.
Nothing renamed; codename "Dev Command Center" remains in all architecture
docs; no repository, organization, domain, commit, or push created.

## R2.1 Strategy change

Round 1 over-favored semantic English words and under-weighted exact
physical-product brand collisions (the PETZL Rollcab miss). Round 2 inverts
priorities: **collision resistance and search distinctiveness first**,
semantic fit second.

Preferred classes: (A) coined names, (B) semi-coined names, (C) uncommon but
pronounceable compounds from specialist vocabularies. Ordinary dictionary
nouns avoided unless exceptionally clean. Crowded patterns banned:
dev-, term-, -mux, -dock, -deck, pane-, grid-, stack-, command-, workspace-,
work-, pilot-, forge-, hub, AI/cloud implications, and simple XDev/TermX/
XDock-style constructions. All fourteen previously forbidden/rejected names
excluded, no superficial spelling variants.

## R2.2 Generation and filtering

**Internally generated: ~90 candidates** across five families: conducting/
coordination (Batuta, Continuo, Tanpura…), joinery/composition (Tenon,
Mortise, Intarsia, Marquetry, Selvage, Dowel, Detent, Kerf, Quillon…),
persistent-place/structure (Nunatak, Strake, Keelson, Garboard, Bolection,
Larmier, Antefix, Antepagment, Bressummer, Pendentive, Impluvium, Forulus,
Basamento, Caementum, Milliarium, Mansio, Castra, Statio, Groma, Allod,
Armarium, Lararium, Kamidana, Tokonoma, Bothy, Drumlin, Tansu…), navigation/
switching (Alidade, Semita, Vereda, Skift…), and archive-keeping
(Scrinium, Bailiwick, Compono…).

**Seriously screened: 58** via package registries (npm, crates.io, PyPI —
HTTP-status verified per name), of which **24 advanced to full web +
GitHub collision screening** with current public sources. 30+ candidates
were filtered pre-screen on known collisions or weak fit and are listed in
§R2.6 for completeness without individual dossiers.

## R2.3 Collision-screen methodology (per candidate)

1. General web: quoted exact name; + software / app / developer / desktop /
   terminal / company.
2. GitHub: exact repos, orgs/users, near-identical active developer projects.
3. Packages: npm, crates.io, PyPI (404 = unregistered at retrieval).
4. Commercial products incl. established physical products — NEW RULE: an
   exact famous/current non-software product is a significant negative;
   different Nice class is NOT treated as clean branding.

Classification: FATAL / HIGH / MEDIUM / LOW per tasking. Only LOW may become
a finalist; a MEDIUM stays in the research table and cannot be PRIMARY.
Search-distinctiveness test applied to every finalist: *"Can this project
plausibly own the first-page identity for its exact word six months after
launch?"*

## R2.4 Screened-out candidates (rejection evidence, Round 2)

| Candidate | Class | Evidence |
|---|---|---|
| quillon | HIGH | Quillon AI (quillon.ai) raised $1.5M Apr 2026, active; Quillon Markets; quillon.partners |
| nunatak | HIGH | The Nunatak Group consultancy (nunatak.com, Munich/Zurich/Berlin); Nunatak Software S.A.; whitmo/nunatak GIS tool |
| armarium | MEDIUM-HIGH | ARMARIUM registered US trademark (#5044268); Armarium fashion brand active (armariumbrand.com); funded startup history |
| allod | FATAL | ALLOD (allod.solutions) active self-hosted secure-web-gateway software; Allod self-custody carrier (allod.tech); multiple GitHub orgs |
| tanpura | HIGH | Multiple music apps named Tanpura (Real Tanpura iOS, Tanpura Studio), tanpura-cli Rust app, several GitHub projects |
| drumlin | FATAL | Drumlin Security Ltd — active PDF DRM software publisher; Drumlin Capital; Drumlin Plasma |
| dowel | HIGH | DOWELL Technologies (est. 1984 IT firm); Dowel Group Paris; dowel.tech CRM SaaS |
| detent | FATAL | Five+ active developer products: pypi `detent` QA tool v1.2.0 (2026), digitaldrywood/detent agent orchestrator, imbue-ai/detent CLI, getdetent.app desktop, Detent Technologies Ltd |
| strake | FATAL | strake-data/strake Rust data runtime (active), strakelabs npm CLI, getstrake.com, strake.dev GitHub Action |
| lararium | FATAL | npm `lararium` ACTIVE agentic-system toolkit (elorati.com, v2.9); Rynaro/lararium Claude Code companion; more repos |
| groma | HIGH | GromaCoin crypto real estate (groma.com); FoundationVision/Groma multimodal LLM (ECCV 2024); groma.cz geodetic software used by Czech cadastral offices; Bitsight Groma Explorer |
| caementum/cementum | HIGH | Marazzi "Cementum" tile collection; Pedrali "caementum" furniture line; CEMENTUM CZ brand; UK/US companies |
| milliarium | MEDIUM | Milliarium LLC advisory firm active (milliarium.org) |
| flotilla | HIGH | Flotilla IoT fleet SaaS; flotilla.app Taiwan dev co; Flotilla Group UK |
| binnacle | FATAL | binnacle-app/Binnacle local-first Cloudflare desktop workbench (2026); binnacle.io logging service + npm client; more |
| pilothouse | HIGH | PilotHouse charter SaaS; Pilothouse Software marine regulatory co; Pilothouse Consulting |
| mansio | HIGH | MANSIO GmbH funded logistics-software startup (2025 seed); identical Roman etymology already in use |
| castra | HIGH | Castra AB Swedish IT consultancy (~330 staff); Castra Technologies cybersecurity; more |
| forestay | FATAL | Forestay® registered US trademark (Forestay Capital, enterprise-AI/SaaS VC) |
| statio | MEDIUM | Statio iOS system monitor; Statio warehouse app (2026); PyPI stats lib; weak distinctiveness vs "station" |
| bothy | FATAL | deakdotdev/bothy developer desktop app; sp00nznet/bothy infra deployment console; Bothy Technology org |
| slipway | FATAL (R1 carry-over re-check) | sailscastshq/slipway deployment platform + npm CLI; slipwayhq Rust framework |
| muster/shipshape/wheelhouse/homeport/toolchest/fairlead/capstan/pilothouse/millwright | FATAL/HIGH | carried over from Round 1 screening (evidence unchanged, §4) |
| windbrace | HIGH | WindBrace® RoyOMartin registered product line; Saturn Maple WINDBRACE steel profiles; Monster Hunter item |
| raggle | FATAL | Raggle Software Pty Ltd Australia (since 1998); raggle.co "developer-friendly AI workflows" active product; pablotron/raggle RSS tool |
| tiebeam | FATAL | Tiebeam Technologies India Pvt Ltd — active software company since 1994; TIEBEAM Ventures tech investor (tiebeam.com) |
| crowstep | MEDIUM | Crowstep internet company listing (ZoomInfo); inactive branding agency (Tracxn) |
| basamento | MEDIUM | Common Spanish dictionary word (plinth); existing boilerplate repo + Madrid firm |
| impluvium | MEDIUM | impluvium-software GitHub org exists; deadpooled skincare brand; Serbian SP company |
| intarsia / marquetry | MEDIUM | craft-domain dictionary dominance fails the first-page-identity test; minor packages exist |
| forulus | FATAL | forulus.com — ACTIVE "local-first medical data vault" app (Devpost Aug 2026) — direct category collision |
| gablet | HIGH | mystborn/gablet comic platform + GabletUI; Gablet INC India; Gablet Solutions agency |
| squinch / stylobate | not finalists | npm name taken (registry 200) |

## R2.5 Finalists (exactly 5 — all classified LOW)

All five are rare structural/architectural terms — thematically coherent
(permanence, load-bearing, protection, framing) — each verified FREE on npm,
crates.io, AND PyPI, with zero current software/product/company exact-name
collisions found in public sources at retrieval time.

### F1 — Garboard
- Product display name: Garboard · lowercase repo slug: `garboard` · CLI/binary: `garboard`
- Pronunciation: GAR-bərd
- Brand rationale: the garboard is the FIRST plank of a hull, permanently
  fixed to the keel — everything else is built upon it. Persistence plus
  foundation in one word.
- Collision classification: **LOW**
- Exact public hits found: nautical glossaries/Wikipedia only; zero brands,
  products, or repos.
- Package signals: npm free · crates.io free · PyPI free
- Domain signals (RDAP): .com/.dev/.app all currently registered (secondary
  per strategy)
- Preliminary trademark screen: no material conflict found in preliminary
  public screen (interactive USPTO/WIPO/EUIPO automation unreliable — see §R2.8)
- Weighted score: **83**

### F2 — Bolection  ★ SECONDARY
- Product display name: Bolection · lowercase repo slug: `bolection` · CLI/binary: `bolection`
- Pronunciation: bə-LEK-shən
- Brand rationale: the bolection is the raised molding that covers and
  bridges joints between surfaces of DIFFERENT levels — precisely what the
  product does for terminals, GUI apps, local and remote tools within one
  frame. Distinctive, refined, unclaimed.
- Collision classification: **LOW**
- Exact public hits found: none beyond architecture dictionaries (searches
  returned only generic IP-information pages)
- Package signals: npm free · crates.io free · PyPI free
- Domain signals (RDAP): bolection.dev **free**, bolection.app **free**, .com registered
- Preliminary trademark screen: no material conflict found in preliminary
  public screen
- Weighted score: **86**

### F3 — Antepagment
- Product display name: Antepagment · lowercase repo slug: `antepagment` · CLI/binary: `antepagment`
- Pronunciation: an-tə-PAG-mənt
- Brand rationale: archaic term for the complete framing of a doorway —
  launchers are doorways to tools; the product builds the frame around them.
- Collision classification: **LOW**
- Exact public hits found: zero (search returns nothing but generic pages);
  fully free everywhere
- Package signals: npm free · crates.io free · PyPI free
- Domain signals (RDAP): antepagment.com/.dev/.app ALL FREE
- Preliminary trademark screen: no material conflict found in preliminary
  public screen
- Weighted score: **79** (memorability/spelling burden caps it)

### F4 — Larmier
- Product display name: Larmier · lowercase repo slug: `larmier` · CLI/binary: `larmier`
- Pronunciation: lar-MYAY (anglicized LAR-meer acceptable)
- Brand rationale: the drip edge that channels water away and protects the
  wall beneath — small, precise, protective detail work; matches the
  review/authorization ethos.
- Collision classification: **LOW**
- Exact public hits found: zero (only GitHub boilerplate pages returned)
- Package signals: npm free · crates.io free · PyPI free
- Domain signals (RDAP): larmier.dev **free**, larmier.app **free**, .com registered
- Preliminary trademark screen: no material conflict found in preliminary
  public screen
- Weighted score: **81**

### F5 — Pendentive
- Product display name: Pendentive · lowercase repo slug: `pendentive` · CLI/binary: `pendentive`
- Pronunciation: pen-DEN-tiv
- Brand rationale: the constructive device that lets a circular dome rest on
  a square room, transferring its weight to four corner piers — the exact
  structural metaphor for letting GUI applications rest on a terminal-native
  core with failure isolation at the corners.
- Collision classification: **LOW**
- Exact public hits found: architecture-education content only (Wikipedia,
  Britannica); zero brands/products/repos
- Package signals: npm free · crates.io free · PyPI free
- Domain signals (RDAP): not individually verified (time-boxed); treat as
  unknown until registration attempt
- Preliminary trademark screen: no material conflict found in preliminary
  public screen
- Weighted score: **83**

Near-miss documented as research-table-only (MEDIUM): **Bressummer** —
completely free on all three registries and zero software collisions, but a
small Winchester (UK) building-surveying firm trades as "Bressummer A.R.K
Ltd"; unrelated smaller commercial use = MEDIUM → ineligible for finalist/
PRIMARY status under Round 2 rules. Scored hypothetically ~86.

## R2.6 Weighted scoring (Rubric R2)

Collision resistance 35 · Search distinctiveness 20 · Memorability/
pronunciation 15 · Product semantic fit 10 · Developer credibility 10 ·
CLI/repository usability 5 · Brand extensibility 5. FATAL/HIGH auto-
disqualifies regardless of score.

| Criterion (max) | Bolection | Garboard | Pendentive | Larmier | Antepagment |
|---|---|---|---|---|---|
| Collision resistance (35) | 34 | 33 | 33 | 33 | 34 |
| Search distinctiveness (20) | 18 | 17 | 16 | 17 | 19 |
| Memorability/pronunciation (15) | 11 | 10 | 9 | 10 | 8 |
| Product semantic fit (10) | 7 | 8 | 9 | 6 | 6 |
| Developer credibility (10) | 7 | 6 | 7 | 6 | 5 |
| CLI/repository usability (5) | 5 | 5 | 5 | 5 | 3 |
| Brand extensibility (5) | 4 | 4 | 4 | 4 | 4 |
| **Total (/100)** | **86** | **83** | **83** | **81** | **79** |

Search-distinctiveness test: all five plausibly own their first page within
six months of launch (each currently returns only dictionary/glossary or
zero content).

## R2.7 Recommendation

- **PRIMARY: Bolection — score 86, collision risk LOW.** Meets both hard-gate
  conditions (≥85 AND LOW). Free on all three package registries;
  .dev and .app domains unregistered at retrieval; no surfaced marks; a
  genuinely distinctive single word whose meaning (molding that bridges
  unequal surfaces into one finished frame) maps directly onto the product's
  core proposition.
- **SECONDARY: Garboard — score 83, collision risk LOW.** Strongest fallback
  on memorability/pronunciation among remaining LOW candidates.

Canonical forms (direction only — nothing renamed):

| Form | Bolection (PRIMARY) | Garboard (SECONDARY) |
|---|---|---|
| Product display name | Bolection | Garboard |
| Repository slug | `bolection` | `garboard` |
| Binary/CLI | `bolection` | `garboard` |
| Tauri identifier direction | reverse-DNS once domain secured (e.g. `dev.bolection.desktop`) | same pattern (`dev.garboard.*`) |
| Config dir direction | `$XDG_CONFIG_HOME/bolection` / `%APPDATA%\bolection` | `$XDG_CONFIG_HOME/garboard` / `%APPDATA%\garboard` |

## R2.8 Trademark screen — results and limitations

Preliminary public screens performed per finalist via current web sources
including USPTO-surfacing aggregators. Result for all five finalists: **no
material conflict found in the preliminary public screen.** Limitations
(stated plainly): the interactive USPTO tmsearch, WIPO Global Brand Database,
and EUIPO eSearch+ could not be reliably queried programmatically in this
pass (JavaScript apps / credentialed APIs; Justia automated-access denial);
findings are indicative, not exhaustive. **No statement here constitutes
"trademark cleared".** Formal counsel clearance in relevant Nice classes
(especially 9 and 42) remains a mandatory later release gate.

## R2.9 Round 2 gate summary

- ~90 names generated; 58 seriously registry-screened; 24 full web/GitHub
  screened; 30+ recorded rejections with evidence.
- Exactly 5 finalists, all LOW-risk; 0 contradictions found against PRD §17.
- Nothing renamed; codename unchanged anywhere; no repository/domain/package
  actions performed.

PRIMARY RECOMMENDATION: **Bolection** (86/100, LOW risk)
SECONDARY: **Garboard** (83/100, LOW risk)

NAMING_GATE_R2=READY_FOR_HUMAN_REVIEW

---

# Round 3 — True Coined Brand Search

Status: COMPLETE, retrieval date 2026-08-26. Research + documentation only;
nothing renamed; codename unchanged; no repo/domain/package actions.

## R3.1 Strategy and generation

Round 2 failed because finalists were uncommon dictionary/architectural
terms. Round 3 mandates TRUE COINED BRANDS only: no English headwords, no
established French/Latin/nautical/architectural terms, no dominant surnames,
no buzzword compounds, no fantasy-game phonology.

Internally generated via phoneme construction and syllable blending:
~160 candidates. After lexical pre-filtering, 32 serious coined candidates
entered screening: vantry, corven, sorven, kelvar, talvin, drevna, fenrik,
lorvic, ravnor, torvin, tavro, kevral, orvel, avrel, kestria, cirvel, imbrel,
nelvar, orvand, quelva, beltra, keltra, reltra, vandro, landris, tandros,
stowen, lathron, dorvan, kaldren, navrik, ilvane.

## R3.2 Registry gate (exact results)

npm / crates.io / PyPI HTTP status per name (404 = unregistered at
retrieval): 31 of 32 completely free on ALL THREE registries; only `vantry`
has a PyPI occupant. Full table retained in this file's edit history; every
finalist below re-verified individually: ravnor 404/404/404 · cirvel
404/404/404 · quelva 404/404/404 · vandro 404/404/404 · nelvar 404/404/404.

## R3.3 Screened-out candidates (evidence)

corven FATAL (Corven Labs tech firm; Corven Group consultancy acquired by
Oliver Wyman; Corven International security; Grupo Corven) · talvin HIGH
(Talvin AI recruitment startup, funded 2025, talvin.ai) · sorven HIGH
(Sorven Global IT services; Sorven Partners Ltd UK) · fenrik LEXICAL FAIL
(real Norwegian military officer rank, Wikipedia-documented; multiple GitHub
users) · tavro FATAL (Tavro AI LLC governance platform tavro.ai + ServiceNow
listing; tavroapp.com job tracking; tavro.io fitness) · torvin FATAL/HIGH
(torvin.com audio brand; torvin.app FP&A SaaS; GitHub user Torvin) · orvel
HIGH (Orvel Ventures orvel.vc; hackathon repos) · avrel MEDIUM (GitHub users;
Saint-Avrel French commune) · kestria FATAL (Kestria global executive-search
organization, kestria.com) · navrik FATAL (Navrik Software Solutions, active
India/USA RPA firm) · drevna MEDIUM (Drevna-hudson Physical Therapy
Associates P.C., active Lancaster PA practice; surname-based business) ·
kelvar HIGH near-name (one letter from KEVLAR® DuPont trademark; autocorrect
domination) · landris HIGH near-name (Landis+Gyr giant; Landry's hospitality
chain) · beltra HIGH near-name (Adrián Beltré surname search dominance) ·
tandros MEDIUM (existing Tibia game NPC with wiki presence) · stowen/lathron/
dorvan/kaldren/ilvane/orvand imbrel keltra reltra lorvic — screened only at
registry level (free); not advanced, superseded by stronger finalists.

## R3.4 Finalists (exactly 5 — all TRUE COINED words, all LOW risk)

Common facts: not English headwords; not established technical/marine/
architectural terms; no dominant surnames; zero current software/company/
product exact matches found; npm/crates.io/PyPI fully unregistered; no
near-name one-letter collisions with established brands after explicit
checks (KEVLAR, Landis, Beltre, CorVel, Android).

### F1 Ravnor ★ PRIMARY
Display: Ravnor · Pronunciation: RAV-nor · Repo slug: `ravnor` · CLI: `ravnor`
Rationale: plosive-first two-syllable coin; reads like an established devtool
brand; zero prior lexical identity anywhere.
Lexical check: coined, no dictionary entry, no term/surname status.
Software/company collisions: none found (web, exact + software/app/dev/
company/GitHub/SaaS). Near-name: none surfaced.
GitHub signal: no org/user/repos with meaningful presence. npm: free.
crates.io: free. PyPI: free.
Domains (RDAP): .dev FREE · .app FREE · .com registered.
Trademark note: no material conflict surfaced in preliminary public screen
(interactive USPTO/WIPO/EUIPO automation unreliable — limitation stated;
not legal clearance).
Collision risk: LOW. Score: 34+24+12+8+4+5+4 = **91**

### F2 Cirvel ★ SECONDARY
Display: Cirvel · Pronunciation: SIR-vel · Repo slug: `cirvel` · CLI: `cirvel`
Rationale: soft, Linear/Vercel-register coin; elegant and memorable.
Lexical check: coined; no dictionary/term/surname status.
Collisions: none found across all seven query classes. Near-name: none.
GitHub: nothing meaningful. npm/crates/PyPI: all free.
Domains (RDAP): .dev FREE · .app FREE · .com registered.
Trademark: no material conflict surfaced (same limitations).
Collision risk: LOW. Score: 33+24+13+8+3+5+4 = **90**

### F3 Vandro
Display: Vandro · Pronunciation: VAN-dro · slug/CLI `vandro`. Coined; no
collisions found; mild Android phonetic echo noted (-1 ownership).
Domains: .dev FREE, .app registered, .com registered. Trademark: no conflict
surfaced. LOW. Score: 33+21+12+7+4+5+4 = **86**

### F4 Quelva
Display: Quelva · Pronunciation: KWEL-və. Coined; only a dormant personal
GitHub user (2 commits, unrelated repo); no products/companies. Domains:
.dev/.app FREE. Trademark: no conflict surfaced. LOW.
Score: 33+22+11+6+3+5+4 = **84**

### F5 Nelvar
Display: Nelvar · Pronunciation: NEL-var. Coined; only a small gaming YouTube
channel shares it; no software/companies. Domains: UNVERIFIED (RDAP rate-
limited 429 — recorded honestly, do not treat as available). Trademark: no
conflict surfaced. LOW. Score: 33+22+11+6+3+5+4 = **84**

## R3.5 Scoring rubric (as mandated)
Collision resistance 35 · Search ownership/distinctness 25 · Pronunciation/
memorability 15 · Developer brand credibility 10 · Product fit 5 · CLI/repo
usability 5 · Extensibility 5. Hard gates precede score; FATAL/HIGH auto-
disqualifies. PRIMARY requires LOW + genuine coinage + ≥90; SECONDARY ≥87.
Search-ownership test: YES for all five (each word currently returns ~zero
relevant results; a moderately successful OSS project would dominate page
one).

## R3.6 Recommendation

PRIMARY: **Ravnor** — 91/100, LOW risk, genuine coined identity, passes all
hard gates. SECONDARY: **Cirvel** — 90/100, LOW risk.

Canonical forms (direction only — NOTHING renamed): display Ravnor /
Cirvel; repo slug `ravnor` / `cirvel`; binary `ravnor` / `cirvel`; Tauri
identifier direction reverse-DNS once domain secured (e.g. `dev.ravnor.*`);
config dirs `$XDG_CONFIG_HOME/ravnor` / `%APPDATA%\ravnor` (resp. cirvel).

No implementation, manifests, lockfiles, commits, remotes, pushes, or repo
rename performed.

NAMING_GATE_R3=READY_FOR_HUMAN_REVIEW

---

## Final Human Naming Decision

- Approved: **ToolOnize**
- Display: ToolOnize
- Repository: toolonize
- Binary: toolonize
- Config direction: toolonize
- Tauri identifier direction: com.toolonize.desktop
- Decision owner: Human Product/Technical Lead
- Date: 2026-08-26
- Status: Approved for repository/product use
- Note: preliminary public collision research is not legal clearance; formal legal/trademark review remains a later release gate if required.

All naming rounds above are preserved as historical research and must not be deleted.

HUMAN_NAMING_GATE=APPROVED
