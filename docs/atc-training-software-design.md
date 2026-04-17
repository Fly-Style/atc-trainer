# ATC Training Software Design

## Goal

Design a Windows-compatible ATC training system written in Rust and split into:

- `server`: central session coordinator and state authority
- `client`: graphical desktop application for trainer and student

The first iteration focuses on a single-airfield training scenario with up to two trainers and exactly one student.

## V1 Frozen Scope

The v1 design is considered frozen around these constraints:

- one airport `AAAA`
- runway `18/36`
- runway length `4000 ft`
- asphalt surface
- one terminal / apron
- one main taxiway
- four runway-vacating taxiways
- CTR square extending `10 NM` north, south, east, and west
- ILS drawn for runway `18` and `36`, with only the active runway shown at once
- one session supports up to `2` trainers
- one session supports exactly `1` student
- one student position per session: `GND` or `TWR`
- trainer uses magic-hash login
- student joins with session hash
- one shared Windows client executable for both roles
- server is authoritative and in-memory only
- scenarios are loaded from external files, with one built-in standard scenery included
- trainer controls all aircraft movement
- student edits operational metadata for assumed traffic
- no built-in voice or text communications
- real time with pause / resume
- up to `30` aircraft in one session

Out of scope for v1:

- AI traffic
- scoring or evaluation
- automatic separation monitoring
- scenario editor
- replay UI
- advanced reconnect UX
- multi-window client
- user account system

## Core Product Idea

The system is intended for ATC students to practice basic aerodrome control and traffic management in a simplified environment.

The trainer controls the training session and manipulates aircraft behavior.
The student uses the client to manage aircraft progress through operational statuses and issue planning data such as runway, SID, and squawk assignments.

The exercise ideology is:

- trainer and student are on the same external voice chat
- trainer and student verbally imitate radio exchange
- student assumes aircraft and assigns labels / control metadata
- trainer controls aircraft movement in the simulation

## High-Level Requirements

### Technology

- Both server and client must be written in Rust.
- Both must run on Windows.
- Server must use `axum` for the HTTP server layer.
- Client should use `iced` for the graphical interface and sector rendering.

### Roles

Both server and client support two primary roles:

- `trainer`
- `student`

Trainer participation model for v1:

- one session supports multiple trainers
- all trainers have the same operational control rights
- trainer actions are trusted and resolved by normal server ordering

Student participation model for v1:

- one session supports exactly one student
- the student occupies exactly one position per session
- allowed student positions in v1:
  - `GND`
  - `TWR`

### Authentication / Session Entry

- Trainer launches the client.
- Trainer logs in using a predefined "magic hash".
- After authentication, trainer can create and start a training session.
- Trainer creates the single student position for the session.
- Student launches the client and joins an existing session using the session hash.
- After joining, the student is attached to the created position.

## First Iteration Scope

The first playable scenario contains a single sector with:

- one airfield
- runway `18/36`
- runway length `4000 ft`
- runway surface `asphalt`
- one terminal / stand area
- one main taxiway
- four runway-vacating taxiways
- one CTR modeled as a square / quadrangle
- CTR boundary extending `10 NM` north, south, east, and west from the airport reference point

The initial scope is deliberately simplified to support training flow before adding more realism.

The trainer scope in v1 includes aircraft manipulation both on the ground and after takeoff.

Scenario setup model:

- a scenario defines predefined aircraft placed at specific airport or airspace locations
- trainer can activate or work with those predefined aircraft during the session
- trainer may also add extra aircraft manually during the session
- there is no built-in scenario editor
- scenarios should be loadable from external files
- one standard scenery should also be compiled into the application as a built-in default

Scenario aircraft placement in v1 uses a fixed list of named start positions.
Examples:

- `stand_1`
- `stand_2`
- `stand_3`
- `hold_18`
- `hold_36`
- `ctr_north`
- `ctr_east`
- `ctr_south`
- `ctr_west`
- `pattern_downwind_18`
- `pattern_base_18`
- `pattern_final_18`
- `pattern_downwind_36`
- `pattern_base_36`
- `pattern_final_36`

## Main User Flows

### Trainer Flow

1. Start client.
2. Log in with magic hash.
3. Create or select a session.
4. Create the single student position for the session: `GND` or `TWR`.
5. Load or enter the session METAR before the session starts.
6. Start the session.
7. Spawn or activate aircraft in the scenario.
8. Manipulate aircraft movement and behavior through trainer commands.
9. Observe student actions in real time.

### Student Flow

1. Start client.
2. Join an existing session using the session hash.
3. Load the same airfield/sector view.
4. See aircraft appear in the traffic manager.
5. Occupy the session's assigned position: `GND` or `TWR`.
6. Assume responsibility for aircraft.
7. Assign operational statuses and planning data.
8. Follow trainer-driven traffic progression.

### Position Scope in v1

#### `GND`

- manage aircraft on stand, pushback, startup, taxi, and runway handoff preparation
- assign runway, SID, and squawk for departures
- hand off aircraft to an abstract next position when appropriate

#### `TWR`

- clear IFR arrival traffic to land
- manage VFR traffic in the traffic pattern
- sequence runway usage for departures and arrivals
- assign runway in work when needed
- hand off aircraft to an abstract next position when appropriate

## Client Responsibilities

The client is a graphical desktop application.

The client is a single executable for both trainer and student.
The visual layout should resemble a radar/ATC workstation rather than a generic business application.

### Core UI Areas

- start / login screen
- sector canvas
- traffic manager panel
- trainer tools panel

Recommended `iced` component structure for v1:

- `StartScreen`
- `SessionScreen`
- `SectorCanvas`
- `TrafficManagerPanel`
- `TrainerToolboxPanel`
- `AircraftLabel`
- `PathDraftList`
- confirmation dialog for destructive actions
- reconnect / error banner

### Visual Direction

The client should look similar in structure to the reference image `docs/img.png`.

Important visual characteristics:

- dark radar-style background
- sector canvas as the dominant central area
- compact operational panels docked around the canvas
- information density should feel like an ATC workstation, not a consumer app
- labels and overlays should prioritize operational readability

Concrete v1 visual language:

- background: very dark charcoal / radar gray
- runway, taxiways, CTR, and ILS: thin muted light-gray lines
- active runway: brighter white linework and runway identifier text
- aircraft marker: small high-contrast point / symbol
- one-minute airborne vector: thin green forward vector
- drafted trainer path: dashed amber polyline until launched
- focused aircraft: brighter label and stronger outline
- assumed aircraft: full data label
- unassumed aircraft: callsign only
- transponder label frame colors:
  - `off`: gray
  - `standby`: white
  - `tara`: green

Recommended v1 layout:

- center: large sector / CTR canvas
- bottom: traffic manager
- trainer-only extra toolbox panel for trainer role

Student layout in v1:

- large sector view
- traffic manager at the bottom
- no extra operational panels

Trainer layout in v1:

- the same sector view and traffic manager
- one additional trainer toolbox for aircraft creation and path control

Recommended `SessionScreen` contents:

- `SectorCanvas`
- `TrafficManagerPanel`
- `TrainerToolboxPanel` for trainer only
- top status strip with session name, student position, active runway, pause / running state, and connection state

### Start / Login Screen

Both trainer and student use the same initial screen.

Trainer flow on the start screen:

- enter trainer magic hash
- authenticate as trainer
- open session creation / session management interface

Student flow on the start screen:

- enter prepared session hash
- join the prepared student session
- enter the live training client in the assigned position

Recommended `StartScreen` contents:

- server profile selector
- host field
- port field
- trainer magic-hash login section
- student session-hash join section
- connection / error message area

### Sector Canvas

The sector view should visually render:

- runway `18/36`
- terminal / apron area
- main taxiway
- four runway exits
- CTR quadrangle
- active-runway ILS depiction
- aircraft positions and movement
- aircraft labels with:
  - callsign
  - aircraft type
  - next waypoint
  - current altitude
  - assigned altitude

`iced` can be used for:

- main window and layout
- custom canvas drawing for the airfield map
- event handling and state-driven redraws

The sector canvas in v1 should support zoom in and zoom out.
The sector canvas in v1 should also support panning.
The sector canvas should support focusing an aircraft label so the traffic manager can edit the selected aircraft.
The sector canvas should draw the ILS for the currently active runway only.
For trainer users, the sector canvas should support placing path points directly on the radar display for the selected aircraft.

Sector coordinate and camera model in v1:

- use a single 2D world coordinate system centered on the airport reference point
- world units are nautical miles
- `x` increases east and `y` increases north
- runway, taxiways, CTR, ILS, aircraft, labels, vectors, and trainer path points use the same world space
- zoom and pan are local client view state, independent for each connected user
- zoom should be performed toward the current mouse position

Trainer path control in v1 is point-based:

- trainer marks points on the radar where the aircraft should move
- each path point carries a target speed
- each path point carries a target altitude, or `GND` when the aircraft should remain on the ground
- trainer first selects an aircraft, then drafts a path for that aircraft only
- trainer places all path points first, then edits their parameters afterward
- the drafted path and ordered points must be visible to the trainer before launch
- trainer may undo only the last drafted point before launch
- trainer uses an explicit `Finish pathing` action before point-parameter editing begins
- after `Finish pathing`, point geometry is fixed for that draft
- after `Finish pathing`, the trainer may edit only point speed and altitude / `GND`
- launch should not add protective validation beyond basic command decoding because trainer actions are trusted in v1
- aircraft movement starts only when the trainer explicitly launches the path
- trainer must be able to stop an aircraft while it is still on the ground
- once a path has been launched, the trainer must stop the aircraft before replacing or editing that path
- the server remains authoritative for following the defined path
- the planned path should be visible on the shared sector display
- aircraft movement between path points should use straight-line interpolation in v1

Active runway behavior:

- trainer may switch active runway between `18` and `36` at any time during the session
- switching active runway updates the visible ILS immediately
- switching active runway auto-reassigns all ground aircraft to the new runway in work, even if no runway had previously been assigned
- if an affected aircraft already has a SID assigned and is still on the ground, the SID is auto-converted to the matching paired SID for the new runway

### Traffic Manager Panel

The traffic manager is a focused aircraft editor, not a table of all aircraft.

Behavior:

- when no aircraft label is in focus, the traffic manager is empty
- when an aircraft label is in focus, the traffic manager shows the selected aircraft details and editable fields

Focus and assumption rules:

- focus is temporary UI state
- assumption is operational state and independent from focus
- changing focus does not release assumption
- the student may assume multiple aircraft
- only one aircraft may be focused at a time

The traffic manager should allow the student to:

- assume the focused aircraft
- view departure airfield, initially `AAAA`
- view destination airfield, initially one of `AAAN`, `AAAS`, `AAAE`, `AAAW`
- view callsign
- view aircraft type
- view flight rules
- view route
- view assigned runway
- assign SID
- assign squawk code
- assign altitude
- assign runway
- set aircraft status
- manage aircraft labels relevant to the assigned position
- perform an explicit handoff to an abstract next position

The traffic manager is not responsible for physically moving aircraft.
Movement remains under trainer control.

Edit permissions:

- student may edit assumed traffic only
- trainer may edit any traffic

Primary interaction model:

- aircraft is selected from its radar / sector label
- the bottom traffic manager reflects the currently focused aircraft only

### Logic Text Area
The previously discussed logic text area is removed from the v1 layout.

### Trainer Tools Panel

Trainer sees additional tools that the student does not.

The panel should include:

- create aircraft
- remove aircraft
- set active runway
- assign runway in work
- set speed
- draft path
- launch drafted aircraft movement
- stop ground aircraft
- go around
- rejected takeoff
- pause session
- resume session

Trainer aircraft creation in v1 is template-based.

Allowed manual-add templates:

- `C172 VFR`
- `A320 IFR`

When manually adding an aircraft, trainer must provide:

- aircraft template
- callsign
- squawk mode
- initial location selected freely anywhere on the radar / sector display
- initial aircraft state
- if the aircraft is created airborne, an initial path must also be assigned

Manual aircraft placement does not snap to predefined airport or airspace points.
The trainer may place created aircraft freely and is assumed to use that capability correctly.
Trainer-created aircraft become visible to the student immediately after creation.

The squawk code itself remains part of student workflow in the traffic manager.
Aircraft type in v1 is fixed by the selected template and may not be overridden manually.

The trainer tools should feel like operational control widgets attached to the same shared view, not a separate mode or separate application.

Unified v1 aircraft status set:

- `new`
- `cleared`
- `push`
- `startup`
- `taxi`
- `on_runway`
- `airborne`

This same list is used for both departure and arrival traffic in v1.
Airborne context is represented through route / next waypoint assignment rather than separate arrival-only states.

### Trainer Controls

The trainer must be able to perform exactly these v1 control actions:

- create aircraft
- remove aircraft
- set active runway
- assign runway in work
- set speed
- draft path
- launch path
- stop ground aircraft
- go around
- rejected takeoff
- pause session
- resume session

Trainer control can begin as a simple command panel combined with direct point placement on the radar display.
Predefined scenario aircraft and manually added aircraft should use the same runtime aircraft model after creation.

Abnormal event handling in v1:

- supported explicit abnormal events are `go-around` and `rejected takeoff`
- `go-around` should send the aircraft to runway heading only
- after the initial runway-heading segment, the trainer vectors the aircraft manually
- `rejected takeoff` should be available when the aircraft is in an on-runway departure state
- rejected takeoff handling may be represented either as a dedicated trainer action or as part of the planned path logic

## Server Responsibilities

The server is the source of truth for sessions and synchronized simulation state.

### Main Responsibilities

- authenticate trainer using magic hash
- create and manage sessions
- allow students to join sessions
- store connected clients and roles
- own authoritative aircraft/session state
- broadcast state updates to clients
- validate trainer and student actions
- run entirely in memory for v1 with no database

Both trainer and student should see the same synchronized client state for a session.
Role differences are defined by available tools and permissions, not by separate session views.

Session liveness rule in v1:

- if the student disconnects, the session continues running
- if all trainers disconnect, the session enters a disconnect cooldown
- if no trainer reconnects within the cooldown window, the session closes automatically
- v1 cooldown target: `3 minutes`
- the server writes session action logs to files
- v2 should support replaying a recorded session from stored logs

Weather handling in v1:

- there is no dynamic weather simulation
- trainer loads or enters the METAR before session start
- the loaded METAR is session briefing data for trainer and student
- runway selection and traffic behavior remain trainer-driven

Pause and resume semantics in v1:

- pausing a session freezes aircraft movement
- viewing and editing aircraft metadata may still continue
- resuming continues aircraft movement from the frozen state

### Suggested Communication Model

Use `axum` for HTTP APIs and add a real-time channel for live updates.

Suggested split:

- HTTP REST endpoints for login, session creation, join flow, and static metadata
- WebSocket channel for session events and real-time state sync

### Suggested Server Modules

- `auth`
- `sessions`
- `aircraft`
- `simulation`
- `transport`
- `state`

## Suggested Architecture

### Workspace Layout

Recommended Rust workspace layout:

```text
atc-trainer/
  Cargo.toml
  crates/
    atc-server/
    atc-client/
    atc-shared/
```

### Crate Responsibilities

#### `atc-shared`

Shared models and protocol types:

- role definitions
- session identifiers
- aircraft identifiers
- sector layout model
- command/event enums
- DTOs for HTTP and WebSocket messages

#### `atc-server`

- `axum` application
- authentication and session management
- authoritative simulation state
- WebSocket hub / broadcaster
- in-memory runtime state only in v1

#### `atc-client`

- `iced` desktop UI
- sector rendering
- traffic manager widgets
- HTTP/WebSocket connectivity
- synchronized shared session view for trainer and student
- local view state and input handling

## Domain Model Draft

### User Role

```text
Trainer
Student
```

### Trainer Role

Fields:

- `trainer_id`
- `connected`

### Student Position

Fields:

- `position_id`
- `position_type` = `GND | TWR`
- `occupied_by`
- `status` = `open | occupied | disconnected`

### Session

Fields:

- `session_id`
- `session_hash`
- `name`
- `created_by`
- `started_at`
- `status`
- `connected_clients`
- `trainers`
- `student_position`
- `sector_id`

### Aircraft

Fields:

- `aircraft_id`
- `callsign`
- `aircraft_type`
- `origin`
- `destination`
- `stand`
- `position`
- `movement_state`
- `student_status`
- `assigned_runway`
- `assigned_sid`
- `assigned_squawk`
- `controlled_by`

### Sector

Fields:

- `sector_id`
- `airport_icao`
- `runways`
- `stands`
- `taxiways`
- `holding_points`
- `spawn_points`

## Initial Simulation Design

The first version should not attempt to model full aircraft physics.
It should instead use a controlled state-driven movement model.

### Proposed Simplification

- Trainer defines movement with radar-placed path points.
- Each path point carries target speed and target altitude or `GND`.
- Aircraft movement starts only after explicit trainer launch.
- Client interpolates position visually with straight-line segments between path points.
- Student actions update operational metadata, not raw movement physics.

This keeps the first version implementable while still useful for training.

## Initial Airport Layout Draft

Single airport graph:

- `RWY18_THRESHOLD`
- `RWY36_THRESHOLD`
- `APRON`
- `TWY_MAIN`
- `EXIT_A`
- `EXIT_B`
- `EXIT_C`
- `EXIT_D`
- `HOLD_18`
- `HOLD_36`

Initial airspace boundary:

- airport-centered CTR
- square / quadrangle shape
- extends `10 NM` to the north, south, east, and west from the airport reference point

### SID Model for v1

SIDs are predefined in airport configuration and intentionally abstract.

- `4` SID families total
- each family has two runway variants
- `A` variant is for runway `18`
- `B` variant is for runway `36`

Suggested identifiers:

- `north1a` / `north1b`
- `east1a` / `east1b`
- `south1a` / `south1b`
- `west1a` / `west1b`

Each SID definition should include:

- `sid_id`
- `runway`
- `label`
- `initial_heading`
- optional `waypoints`
- optional display color

## Scenario Structure

Recommended v1 external scenario format:

- human-editable `TOML`
- one file per scenario

Required top-level fields:

- `scenario_id`
- `name`
- `student_position_type`
- `initial_active_runway`
- `metar`
- `aircraft`

Each scenario aircraft should define:

- `aircraft_id`
- `template`
- `callsign`
- `flight_rules`
- `departure_airfield`
- `destination_airfield`
- `route`
- `initial_spawn`
- `initial_status`
- `squawk_mode`
- optional `assigned_sid`
- optional `assigned_runway`
- optional `assigned_altitude_ft`

Example v1 scenario file:

- [scenario-gnd-36-medium.toml](/Users/sasha/dev/atc-trainer/docs/scenario-gnd-36-medium.toml)

The built-in standard scenery should represent:

- one airport `AAAA`
- runway `18/36`
- runway length `4000 ft`
- asphalt surface
- one terminal / apron
- one main taxiway
- four runway-vacating taxiways
- CTR square extending `10 NM` in each cardinal direction
- ILS depiction for `18` and `36`
- four SID families with paired runway variants

## Networking Draft

### HTTP Endpoints

- `POST /auth/trainer-login`
- `POST /sessions`
- `POST /sessions/{id}/join`
- `POST /sessions/{id}/start`
- `POST /sessions/{id}/student-position`
- `GET /sessions/{id}`
- `GET /sector/{id}`

### WebSocket Events

Client to server:

- `join_session`
- `trainer_command`
- `student_update_aircraft`
- `ping`

Server to client:

- `session_state`
- `aircraft_created`
- `aircraft_updated`
- `aircraft_removed`
- `trainer_notice`
- `error`

## UX Notes

### Trainer UX

- Fast login with magic hash
- Quick session start
- Rapid aircraft creation and manipulation
- Clear visibility into student actions

### Student UX

- Minimal join friction
- Join by session hash
- Clear visual mapping between aircraft and airport layout
- Fast editing of aircraft assignments
- Visible progression of aircraft through training states
- Clear indication of active student position: `GND` or `TWR`

## Non-Functional Requirements

- Windows-first support
- Reasonably low-latency updates for shared session state
- Deterministic state handling on the server
- Crash-safe behavior where invalid client actions do not corrupt session state
- Clear separation between shared protocol and UI/server code

Practical v1 limits:

- maximum `2` trainers
- maximum `1` student
- up to `30` aircraft in one running session

Explicit v1 non-goals:

- no built-in voice or text communication
- no AI traffic
- no automatic separation monitoring
- no scoring or evaluation
- no scenario editor
- no detached multi-window UI
- no replay UI yet
- no advanced reconnect UX
- no account system

## Security Notes

For the first prototype:

- magic hash can be a static trainer secret from configuration
- student join is protected by a session hash
- student identity is session-bound rather than account-bound

For later versions:

- proper user accounts
- secure secret storage
- role-based authorization
- audit log of trainer/student actions

## Roadmap

### Phase 1: Prototype

- create Rust workspace
- define shared protocol types
- implement `axum` server with trainer login and session creation
- implement `iced` client with role selection and session join
- render static single-airfield map
- show synchronized aircraft list

### Phase 2: Interactive Training

- add aircraft creation and movement commands
- add student traffic manager workflows
- synchronize state via WebSocket
- add trainer command panel
- support trainer manipulation after takeoff with simple airborne profiles
- support both departures and arrivals in the same session
- add VFR pattern handling for `TWR`
- add IFR arrival flow with landing clearance states

### Phase 3: Usable MVP

- add validation rules
- improve aircraft routing model
- add session recovery / persistence
- improve UI clarity and ergonomics

### Phase 4: Expanded Simulation

- more airfields
- arrivals and departures
- voice integration
- scripted exercises
- scoring and debriefing

## Open Questions

No open questions remain from the initial concept pass.

Locked decisions:

- v1 airport layout is hardcoded in code
- v2 may load airport layout from external data files
- there will never be a built-in scenario editor
- scenarios may still exist as predefined scenario definitions without an editor

## Recommended First Technical Decisions

- Use a Rust workspace with `server`, `client`, and `shared` crates.
- Use `axum` + WebSocket on the server.
- Use `iced` with canvas drawing on the client.
- Use server-authoritative state sync.
- Start with graph-based airport movement, not free movement physics.
- Keep the first airport hardcoded to reduce implementation complexity.

## Next Design Step

The next useful artifact should be one of:

1. a more formal product requirements document
2. a detailed Rust workspace/module structure
3. a protocol specification for client-server messages
4. a wireframe for the client screens
