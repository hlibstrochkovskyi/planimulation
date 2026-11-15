# Bruno interface study and Planimulation workspace proposal

Date: 2026-10-15. Status: research and a proposed UI direction, not an implemented interface or an amendment to agreed simulation principles.

The subsequent [desktop workspace implementation record](ui-workspace.md) identifies what has now been implemented and how it is checked. The static composition below remains a proposal artifact, not a screenshot of the application.

## Brief and evidence

The owner requested a strict, minimal, professional interface inspired by the Bruno desktop application, with an internet investigation before implementation. The audience is a person exploring generated worlds, inspecting regions, and running supported experiments. The primary job is to keep geography visible while making the current layer, selection, experiment, and saved artifact understandable.

Research used the official desktop screenshot on Bruno's homepage, official release notes, and public component sources. The homepage screenshot identifies itself as v4.1.0. The v3 notes explain the earlier density and hierarchy refresh; they are historical context, not the current version designation. Source inspection is pinned to commit `8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b`, resolved from GitHub on the research date. Sources and screenshot may represent different builds.

This is a visual and source inspection, not a hands-on usability assessment of a running Bruno installation. Claims about remembered pane sizes mean state associated with a request tab; restart persistence was not verified. Proposed dimensions below are Planimulation decisions, not measurements inferred from the marketing image.

Local review covered `DESIGN.md`, `README.md`, `docs/development.md`, `src/renderer/index.html`, `style.css`, `theme.css`, and existing desktop captures. Existing captures illustrate the current structure but are not asserted to show every current uncommitted change.

## What Bruno does, and how to transfer it

| Design move | Evidence in Bruno | Proposed use in Planimulation |
| --- | --- | --- |
| Stable spatial roles | A collection tree, request editor, and response pane share a persistent shell. | Layer navigation, map, region inspector, and experiment controls each get a stable location. |
| Broad quiet surfaces | The official dark screenshot uses continuous dark planes with restrained boundaries. | Let the map be the visual focus. Connect the surrounding tools into panes. |
| Compact type scale | The dark theme defines 11, 12, 13, 14, 16, and 18 px roles; the global body uses its 13 px token. | Start at 13 px for controls and rows; use 12 px for supporting text and 16 px for the selected-region title. |
| Restrained accent | Amber marks the Send action and active inner tabs in the official screenshot. | Use a warm accent for the current primary action and a narrow active marker. Keep scientific field palettes independent. |
| Semantic color | HTTP methods, response success, and syntax have different color roles. | Separate interaction color, execution state, and map data. A green state needs a text label. |
| Distinct tab levels | Document tabs connect visually to their content; request/response inner tabs use lighter treatment. | Use simple local tabs in the inspector and experiment dock. Introduce document tabs only if simultaneous world documents actually exist. |
| Text before container | Much navigation is a row or tab rather than a filled button. | Layers become compact full-width rows. Reserve button chrome for actions. |
| Explicit selected state | Sidebar styles distinguish selection, hover, and keyboard focus; selected items can have a narrow left marker. | The active layer has a marker and subtle fill. Keyboard focus remains independently visible. |
| Small visible divider, large target | Request pane styles use a 1 px divider inside a 12 px drag target. Sidebar resizing has a separate handle. | Offer resizable navigation and inspector panes with an 8–12 px target around the divider. |
| Layout memory | `useTabPaneBoundaries` reads and updates dimensions for the focused request tab. | Remember pane widths and open sections as presentation preferences. Keep them outside simulation checkpoints and reproducibility fingerprints. |
| Controlled density | Sidebar sections constrain overflow and divide available space between expanded sections. | Scroll the layer list independently. Keep layer search and the map toolbar reachable. |
| Quiet interaction | Shared button hover styles change color; the inspected button component has no hover translation or scaling. | Replace our general hover lift and press scaling with restrained color changes. Retain a visible focus ring. |
| Shape reflects role | Theme radii include 4/6/8 px; document tabs have a specific joined silhouette. | Use square pane junctions, 4 px controls, and 6 px menus. Shape conveys role rather than applying one radius everywhere. |
| Contextual work | Request and response remain adjacent, with parameters, body, headers, and results organized locally. | A selected region's values and factor breakdown stay beside the map; experiment configuration stays with its execution controls. |
| Less interruption | The v4 announcement describes moving release notes into a dedicated tab instead of an update modal. | Surface ordinary progress and results inline. Use interrupting dialogs when an action needs a decision, such as discarding unsaved progress. |

The transfer is an interpretation of these observations. It does not establish that every Bruno screen follows every rule. Bruno has shadows, menus, dialogs, and multiple button styles; calling it universally flat or shadow-free would overstate the evidence.

## Current Planimulation friction

1. The 72 px brand header and large atlas heading consume working height before the map tools. The app name, subtitle, stage badge, atlas label, and world name create several competing levels of identification.
2. The 324 px sidebar combines a substantial generation form and roughly thirty layer choices in one scroll flow. Switching a layer can require scrolling past configuration that is no longer the current task.
3. The workspace scrolls as a document. Inspectors and explanations sit after climate controls, two water sections, summaries, display controls, diagnostics, and metrics. Inspecting a region can move its geographic context out of view.
4. Numerous framed sections, 7–10 px corner radii, and 13–23 px padding make operational controls read like equally weighted cards.
5. The seasonal section exposes five initialization variants as peer buttons before the user has selected an experiment. Execution, configuration, checkpoint operations, and explanatory text all compete there.
6. Large metric values include generation timing and the initial fingerprint. These are useful details, but their prominence is disproportionate to the exploration task.
7. Small uppercase labels, tracking, monospace metadata, and decorative arrows add visual texture without consistently helping navigation.
8. The generic hover rule lifts buttons and adds a shadow. This makes a dense instrument feel less spatially steady.

The existing controls already expose useful capabilities. Reorganization must preserve precise names and distinctions: initial water, manual water, evolving seasonal water, climate normals, diagnostic diffusion, recipe files, and full checkpoints represent different things.

## Design plan

### Direction

A compact desktop atlas with persistent scientific inspection. The generated geography provides the memorable visual; the surrounding controls use a disciplined instrument-panel vocabulary.

Core palette, proposed rather than copied as a complete Bruno theme:

| Token | Value | Role |
| --- | --- | --- |
| Workspace | `#1A1A1A` | Main chrome base |
| Panel | `#202020` | Navigation and inspector |
| Control | `#292929` | Inputs and hover fills |
| Divider | `#3A3A3A` | Structural boundaries |
| Text | `#D4D4D4` | Ordinary readable content |
| Accent | `#E4AD43` | Current primary action and active marker |

Supporting text starts at `#A6A6A6`; strong text can use `#EEEEEE`. Success, warning, and error have additional semantic tokens and text labels. Map colors describe data and remain independent of chrome tokens. Contrast must be reviewed for each actual foreground/background pair during implementation; these values are a starting proposal, not a completed accessibility certification.

Typography: bundle Inter locally for the interface, with 400/500/600 weights. Bruno uses Inter in its global font stack; that directly matches the owner's reference. Use tabular numerals for measured values and a monospace face only for hashes, raw identifiers, and raw exports. The current stylesheet merely names Inter; the redesign should make font availability explicit. Start with 13/20 px body text, 12/18 px supporting text, 14/20 px panel headings, and 16/22 px selection titles. Use sentence case and left alignment; align numeric values to the right within inspection rows.

Spacing: 4/8/12/16/24 px. Layer rows start at 30 px; regular controls at 30–32 px. Keep the row height adjustable for interface scaling. Pane boundaries have zero radius; controls 4 px; menus 6 px. Use shadow to separate a temporary overlay from its background, not to outline every permanent pane.

### Layout alternatives

Option A: keep the present document structure and make its components denser.

```text
generation | map
layers     | controls
           | inspection
```

This is a small migration, but the inspector still competes with the map for vertical scroll position. It does not solve the principal exploration problem.

Option B: persistent atlas workspace, recommended.

```text
application and current world                         file actions
Layers        | map toolbar                         | Inspector
search        |                                     | Overview / Factors
Terrain       |                                     | selected region
Water         |              MAP                    | values and units
Climate       |                                     | relevant breakdown
              | layer legend                        |
              +-------------------------------------+
              | experiment selector and controls    |
              | state / progress / budget details   |
status and model identity
```

At 1440 px, start with a 248 px left pane and a 320 px inspector. The map uses the remaining 872 px. Begin with a 40 px app row, a 40 px map toolbar, and a 24–28 px status row. The bottom experiment dock starts at 120–160 px and can collapse. These are tunable bounds, not a promise of one fixed layout at all sizes.

The map/inspector split has the larger target divider described above. Each side pane scrolls independently. The canvas remains visible when a long inspector section scrolls. At approximately 1100 px and below, offer the inspector as a toggled overlay; at smaller widths collapse the layer pane as well. Preserve map access at the currently supported 760 px minimum. Exact breakpoints depend on long labels and actual controls, not device names.

### Review against the brief

The initial idea of adding a narrow icon rail, world-document tabs, and a large timeline would resemble a generic developer workbench and imply unsupported workflows. Remove those from the first proposal. Use labeled navigation, the existing single accepted world, and a dock for the currently implemented experiment controls. Replay, branch comparison, multi-world tabs, and event history remain future concepts under `DESIGN.md`.

The proposed dark neutral palette and amber accent deliberately follow the owner's Bruno reference. Distinction comes from the working material: geographic layers, an uninterrupted map, quantities with units, and model explanations. A new decorative visual identity would conflict with the requested restraint.

## Proposed task flows

### Build and open a world

Move the generation form into a dedicated New world panel reached from a labeled command. Keep its fields and advanced settings. This panel edits a recipe draft, not the accepted world. Generate invokes the current native transaction. Opening a recipe and opening a simulation checkpoint retain separate commands and recognizable artifact names. A file menu may group them, but must not collapse them into an ambiguous Save command.

Keep cancellation, native errors, and the accepted world's continued visibility. An applied generation or checkpoint replacement should reset obsolete selection and inspection data together. If unsaved simulation progress would be lost, explain that consequence at the action that replaces it.

### Choose a layer

The left pane opens with Layers and a text filter, followed by collapsible Terrain, Water, Climate, and Diagnostics groups. The filter matches meaningful names; it does not change the active experiment. Units appear quietly on the right. Keep a selected-layer marker and a title in the map toolbar.

Existing but currently unavailable layers need a short reason, for example “Initialize seasonal water.” Keep those reasons available by keyboard, not only by pointer hover. Avoid displaying future, unimplemented layers as disabled product promises. Region boundaries remain a separate overlay toggle.

### Inspect a region

Clicking the map updates the persistent right pane. Overview presents quantities relevant to the current layer; Factors presents existing model contributions and recorded transfers. Show units, the source frame/time when applicable, and whether a value is initial, prescribed, or evolving. The compact layout must not imply that climate normals and evolving water share a clock.

Before selection, use a clear empty state with one instruction. After selection, keep the region identity stable when switching layers. Existing basin parent/owner actions and exact received high/low components remain reachable in contextual sections. The Factors tab only explains data the model actually provides; it is not a new causal inference system.

### Run an experiment

Group the existing seasonal variants under one Model selector and one Initialize action. Show the chosen mode's scope beside initialization. Retain a separate manual-water mode with its explicit input actions and no model clock. Diagnostic diffusion also retains its separate step counter. Changing the selector must not silently replace or reinterpret an initialized run; use the existing regenerate-to-switch restriction and show its reason.

Once initialized, promote Advance and Run/Pause, show the model's accepted elapsed time, and expose Time per request. During an outstanding request, show that work is pending and preserve the current accepted frame. If Pause stops scheduling but an interval is still in flight, say “Finishing current interval”; do not report the run fully paused before acceptance. Offer only checkpoint commands supported by that mode and disable them while the model is not in a saveable state.

The first layout has no draggable historical time scrubber. The current application has forward evolution and mode-specific checkpoints; a full timeline would imply replay and navigation that are not implemented.

## Implementation mapping and sequence

| Existing element | Proposed destination |
| --- | --- |
| `.app-header`, `.map-heading` | Compact app row and map toolbar |
| `#recipe-form`, `.advanced-settings` | Dedicated world creation panel |
| `.layer-groups`, `#boundaries` | Left layer navigator and overlay section |
| `#map-container`, `.legend`, `#boundary-legend` | Flexible center viewport and its legend |
| Region inspector and explanation blocks | Right inspector with contextual sections |
| `.seasonal-actions`, manual `.water-actions`, `.diagnostic-controls` | Explicit experiment modes in bottom dock |
| `.temperature-controls` | Controls scoped to climate-normal layers |
| `.relief-controls` | Globe display settings |
| `.metrics`, `.world-summary`, `.stage-note` | World details and model scope views |
| `footer` | Concise status and model identity; detailed versions on demand |

1. Establish tokens, typography, compact control variants, and quiet hover/focus states.
2. Introduce the persistent pane layout, preserving existing IDs and controller bindings where practical. Make map resize/picking respond correctly to the new available area.
3. Relocate world creation and group layers; add filtering, keyboard navigation, and pane resizing.
4. Move inspectors into contextual sections and reorganize existing experiment controls with explicit state transitions.
5. Review complete states at 1440×900, 1280×800, and 1024×768 plus the supported narrow limit and increased interface scale: initial, generating, selected, initialized, running, finishing an interval, failed, and graphics recovery. Check long labels, units, unavailable-state reasons, focus order, and overlay dismissal.

This sequence is a proposal. No production renderer, model, checkpoint format, or test was changed or run for this study. Future validation should cover layout and existing actions without rerunning unrelated numerical studies for cosmetic changes.

## Visual study

[Workspace composition study](data/ui-workspace-study.svg) is a standalone static SVG, with a [PNG export](data/ui-workspace-study.png). It embeds an existing map capture from `artifacts/geography-uplands-map.png`, cropped to the geographic viewport; the map is not a new render or new simulation output. Controls, pane arrangement, and empty inspector are proposed. It has no interaction and does not represent shipped functionality. The exported composition was visually reviewed in Chromium.

## Sources

- [Official Bruno homepage](https://www.usebruno.com/) and [desktop screenshot](https://www.usebruno.com/bruno-homepage-hero.png): spatial composition, navigation, action and data colors, tabs, and visible hierarchy.
- [Bruno v3 release notes](https://www.usebruno.com/v3-release): declared refresh goals around compactness, spacing, hierarchy, and responsive layout.
- [Bruno v4 announcement](https://blog.usebruno.com/bruno-v4-0-0-native-ai-typed-variables-and-apps): contextual work and the change from interrupting release notes to a dedicated tab.
- [Dark theme at inspected commit](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/themes/dark/dark.js): palette roles, type sizes, borders, and radii.
- [Global theme styles](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/globalStyles.js) and [global CSS](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/styles/globals.css): base font size and Inter font stack.
- [Sidebar styles](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/components/Sidebar/StyledWrapper.js): selection/focus treatment, constrained sections, and resizing.
- [Request pane styles](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/components/RequestTabPanel/StyledWrapper.js) and [pane state hook](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/hooks/useTabPaneBoundaries/index.js): divider targets, alternative orientations, per-tab pane dimensions, and collapse state.
- [Request tabs](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/components/RequestTabs/StyledWrapper.js), [local tabs](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/components/Tabs/StyledWrapper.js), and [shared buttons](https://github.com/usebruno/bruno/blob/8b955ce91bfe3fd65337ff83bb1f8a17f9b0ae5b/packages/bruno-app/src/ui/Button/StyledWrapper.js): role-specific component treatments.
