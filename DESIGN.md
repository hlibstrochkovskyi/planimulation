Every major feature should emerge from simpler systems whenever possible.

# Planimulation design

Status: concept record from September 19, 2026. The principles below capture the agreed direction. Equations, coefficients, field names, and implementation choices remain proposals until implemented and validated.

## 1. Identity

The user is an explorer of generated worlds. They create a planet, observe change, inspect geography and history, investigate explanations, and compare alternative scenarios.

The long-term causal chain is:

```text
initial conditions → geology → terrain → climate and water → ecology
                                                               ↓
                                       resources → population and settlements
                                                               ↓
                                  routes → exchange → production → states
                                                               ↓
                                 technology, culture, conflict, and history
                                                               ↓
                                          civilization changes its environment
```

Once interactions exist, this becomes a system of feedback loops. Trade supports roads, which support trade. Vegetation affects water retention, which affects vegetation.

The world's appeal comes from explainable differences and unexpected history. A rich deposit does not guarantee a wealthy town; drought does not guarantee war; fertile land does not guarantee state formation.

## 2. Agreed development sequence

1. Generate an Earth-like planet and display its map.
2. Establish natural dynamics: seasons, moisture, runoff, soil, and vegetation.
3. Introduce small population groups, movement, storage, and persistent habitation.
4. Develop settlements, production, routes, trade, and infrastructure.
5. Add states, technology, culture, conflict, and long-term history.

Both civilization starts are desirable: dispersing population groups and established early settlements. They should be different initial states of a compatible model. Dispersing groups remain the first human case; the immediate development target is the natural world without population.

## 3. World laws

### Causality and randomness

Randomness selects initial conditions and bounded disturbances. Persistent relationships determine consequences. Weather has spatial coherence and duration; independently randomizing each region's climate every step is unsuitable.

An event combines susceptibility, a trigger, and environmental state. An ignition source may be stochastic, while fire spread depends on fuel, moisture, and wind.

### Stocks and flows

Changes in a stock have explicit inputs and losses. Water, food, and extracted material must not appear through unrecorded value corrections. An external reservoir is an acceptable approximation if exchanges with it are included in the accounting.

Prescribed or empirical temperature models do not constitute a complete energy balance. Each model must identify what is quantitatively conserved and what is approximated.

### Bounded rates and memory

Heating, drying, vegetation recovery, learning, and construction take time. Recovery can lag behind the end of a disturbance. A region's current condition can depend on previous seasons.

### Geographic continuity

The planet is closed. A flat map's seam is not a barrier to water, air, routes, or influence. Calculations use physical area and distance.

### Multiple timescales

Geological prehistory is generated before the observed history. Weather and water update frequently; ecology changes more slowly; geology operates at a separate scale when needed. Frequencies follow numerical stability and computational cost. Updating every process hourly is not a requirement.

### Reproducibility

A world's identity includes its seed, resolution, algorithm versions, and fully resolved parameters. A seed alone is insufficient after model changes. Subsystems need independent reproducible random streams. Rendering changes must not change simulation outcomes.

The initial exact-reproduction promise applies to one version and a supported execution environment. Bitwise agreement across platforms requires separate verification and is not promised in advance.

## 4. Natural foundation

### Planetary conditions

Conceptual parameters include radius, rotation, axial tilt, orbit, heating conditions, atmospheric properties, water inventory, and geological prehistory. The initial family is limited to worlds compatible with familiar ecological processes. Extreme scenarios are identified separately.

Parameters must be consistent: ocean coverage follows water and terrain; mean temperature follows the heating model. User targets such as ocean fraction are explicitly converted into underlying parameters.

### Geology and terrain

Large-scale forms have structural origins: crustal regions, plate boundaries, uplift, and depressions. Local noise supplements these structures. Geological features later inform the distribution and properties of deposits.

A simplified tectonics-inspired generator is not presented as a numerical mantle simulation or a scientifically reconstructed geological history.

### Water and erosion

Stores include ocean water, atmospheric moisture, snow and ice, surface water, soil moisture, and simplified groundwater. Flows include evaporation, precipitation, melting, infiltration, surface runoff, and groundwater discharge.

Rivers receive water from catchments. Depressions have capacities, water levels, and spill thresholds. Seasonal rivers and closed-basin lakes are possible. Lake surface elevation differs from bed elevation.

Erosion transports material and changes terrain. Sediment deposition can eventually alter drainage and harbor access. Initially erosion prepares the generated world; slow ongoing erosion is a later extension.

### Climate and weather

Seasonal heating, elevation, land/water thermal memory, moisture transport, and terrain interactions create temperature and water regimes. The proposed early model uses prescribed seasonal circulation and coherent disturbances.

Full atmospheric fluid dynamics and complete ocean circulation are deferred. Their absence must be clear when interpreting results.

### Ecology

A biome describes a resulting state. Underlying quantities include biomass, vegetation type, soil moisture, soil depth, a fertility approximation, growth, and recovery. A brief disturbance need not immediately change biome classification.

Vegetation affects water retention and erosion susceptibility. Feedback strength must be checked for instability and excessive amplification.

### Resources

| Type | Examples | Important properties |
| --- | --- | --- |
| Renewable | Forest, fish, wild food, crops | Stock, productivity, recovery, seasonality |
| Finite stock | Ore, coal, other deposits | Quantity, quality, depth, development cost |
| Flow | River discharge, wind, solar energy | Available power or supply over time, seasonality |

Existence, discovery, technical accessibility, and economic viability are separate properties. Transport, fuel, and technology affect resource use.

### Natural events and long-term change

The concept includes droughts, floods, fires, volcanism, earthquakes, and consequences of underwater events. Each is introduced when its causal mechanisms exist. Long-term climate shifts, volcanic cooling, and eventual emissions effects are later extensions.

## 5. First human case

Population, habitation site, and political affiliation are separate. People are represented as aggregate groups rather than individual agents.

A group has population, needs, available labor, portable stocks, and geographic knowledge. It allocates labor to gathering, storage, improvement, exploration, and movement. Extraction reduces a shared local stock also used by other groups.

Movement takes time and food. Carrying capacity is limited, and structures remain at their locations. Knowledge of neighbors and resources is incomplete and becomes outdated. Food spoils, and winter needs differ from summer needs.

Sedentism emerges gradually through repeated returns, storage, longer residence, and accumulated improvements. Stable seasonal routes and different ways of life can coexist.

Agriculture extends the same logic: present labor and resources are committed to a delayed harvest with climatic risks. The first study examines how seasonality, storage, and local depletion affect movement and persistent habitation.

## 6. Civilization: preserved directions

These mechanics belong to the long-term concept, not the first generator.

| System | Core logic |
| --- | --- |
| Settlements | Water, food, access, safety, and accumulated investment affect growth; large cities depend on supply networks |
| Demography | Births, deaths, and migration respond to provisioning, health, safety, and opportunities |
| Production | Limited labor, inputs, tools, and time are converted into goods |
| Markets | Consumption, inventories, demand, supply, purchasing capacity, and transactions shape prices; prices do not create goods |
| Trade | Viability includes transport, travel time, risk, spoilage, capacity, and buyer budgets |
| Routes | Travel costs reflect terrain, rivers, seasons, and safety; infrastructure develops when traffic justifies it and requires maintenance |
| Specialization | Production, exchange, and employment give rise to agricultural, mining, port, industrial, and other centers |
| States | Defense, infrastructure management, trade, and conquest create political organizations |
| Control and borders | Travel time, population, roads, culture, administration, and military presence determine effective influence; formal borders do not imply full control |
| Diplomacy | Goals and dependencies motivate agreements, tariffs, restrictions, alliances, threats, and conflict |
| War | Logistics, mobilization, destruction, debt, and labor losses matter; decisions use incomplete information |
| Technology | Knowledge, ability to apply it, and adoption are distinct; specialists, inputs, training, and infrastructure are needed |
| Culture | Procedural cultural traits spread through contact, migration, and governance; geography shapes connectivity |
| Environmental feedback | Clearing, farming, irrigation, mining, restoration, and eventual emissions alter the natural environment |

Technology should preferably represent capabilities such as agricultural, mining, transport, navigation, medical, and administrative effectiveness. Practice, learning, and exchange support diffusion; isolation and economic destruction can constrain it.

Uneven development is acceptable when its causes can be traced. Bounded rates, costs, and dependencies constrain implausible jumps. Do not automatically equalize societies or introduce arbitrary catch-up effects.

Conflict is not an inevitable response to scarcity. Actors can seek suppliers, change production, negotiate, migrate, or use force. Trade can create both mutual benefit and strategic vulnerability.

## 7. Observation and experimentation

A layered map with navigation and an inspector is the primary interface. Eventually the same model should support a flat map, a globe, and a local 3D view.

Natural layers include physical terrain, elevation, geology, temperature, precipitation, wind, catchments, snow, soil moisture, biomes, productivity, and resources. Later layers include population, settlements, routes, trade, wealth, politics, culture, migration, military conditions, and food security.

The inspector explains a selected region or entity. WHY exposes factors used by the model, recorded flows, and change history. Observed correlation is not presented as proven causation. Explaining a decision differs from proving that no alternative was possible.

Events record time, participants, location, measurable changes, and links to preceding events or conditions. An event must not depend recursively on its own future. A full causal event graph is introduced gradually; generation initially needs value provenance and contribution breakdowns.

The timeline uses checkpoints and reproducible replay. An experimental mode branches from a saved state and changes a condition. Comparing branches helps test explanations, but conclusions apply to this model.

Summary metrics and seed ensembles cover climate, water stores, and productivity; later population, production, trade, migration, states, and conflict. Long-term Monte Carlo mode compares distributions across seeds and scenarios rather than selecting attractive examples.

## 8. Persistence and scale

A generation recipe and a complete simulation checkpoint are different artifacts. Dynamic saves include versions, time, parameters, random-generator states, stores, hidden accumulators, queued actions, and everything required to resume.

Natural fields and future entities update independently of frame rate. Headless execution and batch experiments are required directions. Previously discussed CLI examples describe intended workflows, not an existing interface.

Dense natural fields use arrays; roads, exchange, and migration use graphs; groups, settlements, and states use entities. Rendering resolution may exceed model resolution, but decorative detail cannot create resources or traversable paths.

## 9. Early scope boundaries

Individual humans, detailed battles, political parties, genetics, real languages and religions, thousands of goods, and full atmospheric/oceanic fluid dynamics are deferred. First validate interactions among a small set of understandable processes.

Early outputs are not scientifically validated forecasts of climate or human history. Invariants, controlled experiments, and ensembles check behavior; realism additionally requires comparison with observations.

## 10. Open decisions

- Supported parameter ranges and the distinction between ordinary worlds and experimental scenarios.
- Specific heat, moisture, and ecology equations, units, and calibration.
- Surface resolution and generation/simulation budgets on the target device.
- Initial hydrology depth and lake representation.
- Map and local 3D visual style.
- Numerical tolerances for reproducibility and save portability.

Working proposals appear in [world generation](docs/world-generation.md) and the [implementation plan](docs/implementation-plan.md). They do not override agreed product principles.
