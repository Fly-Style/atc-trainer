# ATC Training Protocol Specification

## Purpose

This document defines the v1 client-server protocol for the ATC training system.

It is intended to drive:

- `atc-shared` Rust types
- `axum` HTTP route design
- WebSocket event contracts
- client authority and validation rules

The protocol is designed for the locked v1 product scope:

- Rust server and Rust client
- Windows-compatible desktop client
- one shared client executable for trainer and student
- one session supports `N` trainers
- one session supports exactly `1` student
- the student occupies exactly one position: `GND` or `TWR`
- trainer login uses a trainer magic hash
- student join uses a session hash
- airport layout is hardcoded in v1
- server is in-memory only in v1

## Transport Model

Use two transports:

- HTTP for login, session creation, join, initial metadata fetch
- WebSocket for real-time state sync and commands

Real-time synchronization model in v1:

- HTTP is used only for login, session creation, join, and initial bootstrap
- WebSocket is used for all live session data exchange
- after WebSocket connect, the client receives an initial full session snapshot
- subsequent live updates are delivered as incremental WebSocket events
- the server may resend a fresh full snapshot over WebSocket if resynchronization is needed

Suggested base URLs:

- HTTP: `/api/v1/...`
- WebSocket: `/api/v1/ws`

The shared client start screen uses:

- trainer magic hash to authenticate and open session creation / management flow
- session hash to join an already prepared student session

## Serialization

Use JSON for all HTTP payloads and WebSocket messages in v1.

Suggested Rust libraries:

- `serde`
- `serde_json`
- `uuid`
- `time`

Conventions:

- enum values serialized as `snake_case`
- timestamps serialized as RFC 3339 UTC strings
- IDs serialized as strings

## Authority Model

The server is authoritative for:

- session existence and lifecycle
- role binding
- trainer and student permissions
- aircraft state
- aircraft movement state
- accepted status transitions
- broadcast ordering of canonical updates
- file-based action logging

Operational split in the training exercise:

- trainer controls aircraft movement
- trainer sets aircraft speed and path
- student assumes aircraft and manages labels / operational metadata
- trainer and student use external voice chat for simulated radio exchange

Clients may optimistically render local intent, but must reconcile to server state.

Both trainer and student clients render the same synchronized session state.
Differences between roles are permission-based:

- trainer has aircraft creation and movement tools
- student has aircraft assumption and label/status tools

All trainers have the same operational rights in v1.
Concurrent trainer actions are resolved by normal server ordering and written to file logs.

Traffic manager interaction model in v1:

- the client focuses one aircraft at a time from the radar / sector label
- when no aircraft is focused, the traffic manager is empty
- when one aircraft is focused, the traffic manager shows that aircraft's editable operational fields

## Identity And Tokens

### Trainer Authentication

Trainer authentication is based on a configured trainer secret hash.

Result:

- successful login returns a `trainer_token`
- token is used for subsequent trainer HTTP requests
- token is also presented during WebSocket connect

### Student Authentication

Student authentication is session-bound.

Result:

- student provides `session_hash`
- server validates the session hash and the existence of the single student position
- successful join returns a `student_token`

### Token Scope

- `trainer_token` is role-scoped and may outlive a single session
- `student_token` is bound to one session and one student position

For v1, opaque random tokens are sufficient.

## Session Lifecycle

Session states:

- `draft`
- `waiting_for_student`
- `running`
- `paused`
- `ended`

Expected lifecycle:

1. trainer logs in
2. trainer creates session
3. trainer creates the student position
4. student joins with `session_hash`
5. trainer loads or enters session METAR
6. trainer starts session
7. trainers manipulate traffic while student works the assigned position
8. session may be paused or ended

Pause semantics in v1:

- pausing freezes aircraft movement only
- aircraft metadata remains visible and may still be edited
- resume continues movement from the frozen state

Disconnect liveness rule:

- if the student disconnects, the running session continues
- if all trainers disconnect, the session starts a trainer reconnect cooldown
- if no trainer reconnects before the cooldown expires, the session transitions to `ended`
- recommended v1 cooldown: `3 minutes`

Server runtime model in v1:

- each session owns one in-memory authoritative state container
- each session processes trainer and student actions through one serialized event loop
- action ordering inside a session is deterministic
- multiple trainers are coordinated by normal event ordering rather than by role restrictions

## Core Domain Types

The following is the protocol-level domain model, not final Rust code.

### Role

```json
"trainer"
```

```json
"student"
```

### StudentPositionType

```json
"gnd"
```

```json
"twr"
```

### SessionStatus

```json
"draft"
```

```json
"waiting_for_student"
```

```json
"running"
```

```json
"paused"
```

```json
"ended"
```

### AircraftCategory

```json
"vfr_c172"
```

```json
"ifr_a320"
```

### FlightRuleType

```json
"vfr"
```

```json
"ifr"
```

### AircraftStatus

Unified v1 statuses for both departure and arrival traffic:

```json
"new"
```

```json
"cleared"
```

```json
"push"
```

```json
"startup"
```

```json
"taxi"
```

```json
"on_runway"
```

```json
"airborne"
```

### MovementMode

```json
"parked"
```

```json
"pushback"
```

```json
"taxiing"
```

```json
"holding"
```

```json
"lineup"
```

```json
"takeoff_roll"
```

```json
"airborne"
```

```json
"landing_roll"
```

```json
"vacating"
```

### RouteNodeId

Route node IDs are hardcoded identifiers in v1.

Examples:

- `apron`
- `twy_main`
- `hold_18`
- `hold_36`
- `rwy18_threshold`
- `rwy36_threshold`
- `exit_a`
- `exit_b`
- `exit_c`
- `exit_d`

### ScenarioPlacementId

Named fixed placements for scenario aircraft in v1.

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

### SidId

SID families are directional procedures with runway-specific variants.

Examples:

- `north1a` for runway `18`
- `north1b` for runway `36`
- `east1a` for runway `18`
- `east1b` for runway `36`
- `south1a` for runway `18`
- `south1b` for runway `36`
- `west1a` for runway `18`
- `west1b` for runway `36`

## Canonical Objects

### SessionSummary

```json
{
  "session_id": "sess_01",
  "name": "Evening training",
  "session_hash": "join-7H4K9P",
  "status": "draft",
  "student_position_type": "gnd",
  "student_connected": false,
  "lead_trainer_connected": true
}
```

### SessionState

```json
{
  "session_id": "sess_01",
  "name": "Evening training",
  "session_hash": "join-7H4K9P",
  "status": "running",
  "sector_id": "airport_v1",
  "student_position": {
    "position_id": "pos_01",
    "position_type": "gnd",
    "occupied": true,
    "connection_state": "connected"
  },
  "trainers": [
    {
      "trainer_id": "tr_01",
      "trainer_kind": "lead_trainer",
      "connected": true
    }
  ],
  "aircraft": [],
  "server_time": "2026-04-17T15:00:00Z",
  "revision": 42
}
```

### AircraftState

```json
{
  "aircraft_id": "ac_01",
  "origin": "scenario",
  "callsign": "ABC123",
  "departure_airfield": "AAAA",
  "destination_airfield": "AAAE",
  "category": "ifr_a320",
  "aircraft_type": "A320",
  "flight_rules": "ifr",
  "route": "RWY18E",
  "status": "taxi",
  "movement_mode": "taxiing",
  "controlled_by_position": "gnd",
  "squawk_mode": "standby",
  "assigned_runway": "18",
  "assigned_sid": "east1a",
  "assigned_squawk": "4123",
  "assigned_altitude_ft": 5000,
  "current_node": "twy_main",
  "target_node": "hold_18",
  "scenario_placement": "stand_1",
  "next_waypoint": "HOLD_18",
  "ground_speed_kt": 14,
  "air_speed_kt": null,
  "altitude_ft": 0,
  "x_nm": -0.4,
  "y_nm": 0.8,
  "trainer_profile": "standard_departure_ifr_a320",
  "revision": 7
}
```

## HTTP API

All endpoints are shown under `/api/v1`.

### `POST /auth/trainer-login`

Authenticate trainer with the configured magic hash.

Request:

```json
{
  "trainer_hash": "sha256:..."
}
```

Response `200`:

```json
{
  "trainer_token": "tok_trainer_123",
  "trainer_id": "tr_01",
  "role": "trainer"
}
```

Failure:

- `401 unauthorized`

### `POST /sessions`

Create a new session.

Auth:

- requires `Authorization: Bearer <trainer_token>`

Request:

```json
{
  "name": "Evening training"
}
```

Response `201`:

```json
{
  "session": {
    "session_id": "sess_01",
    "name": "Evening training",
    "session_hash": "join-7H4K9P",
    "status": "draft",
    "student_position_type": null,
    "student_connected": false,
    "lead_trainer_connected": true
  }
}
```

### `POST /sessions/{session_id}/student-position`

Create the single student position for the session.

Auth:

- requires trainer token

Request:

```json
{
  "position_type": "gnd"
}
```

Response `200`:

```json
{
  "position_id": "pos_01",
  "position_type": "gnd",
  "status": "open"
}
```

Validation:

- reject if a student position already exists
- reject if session already ended

### `POST /sessions/{session_id}/start`

Start the session.

Auth:

- requires trainer token

Request:

```json
{}
```

Response `200`:

```json
{
  "status": "running"
}
```

Validation:

- reject if no student position exists
- reject if student is not connected, unless you later decide offline start is allowed

### `POST /sessions/join-student`

Join a session as the student.

This route avoids exposing `session_id` before join.

Request:

```json
{
  "session_hash": "join-7H4K9P"
}
```

Response `200`:

```json
{
  "student_token": "tok_student_456",
  "session_id": "sess_01",
  "position_id": "pos_01",
  "position_type": "gnd"
}
```

Validation:

- reject if session hash is invalid
- reject if no student position exists
- reject if student position is already occupied

### `GET /sessions/{session_id}`

Fetch current session snapshot.

Auth:

- trainer token for trainers
- student token for the bound student

Response `200`:

- returns `SessionState`

### `GET /sector/{sector_id}`

Fetch hardcoded sector metadata used to initialize rendering.

Auth:

- trainer or student token

Response `200`:

```json
{
  "sector_id": "airport_v1",
  "airport_name": "Training Airport",
  "airport_reference_point": {
    "x_nm": 0.0,
    "y_nm": 0.0
  },
  "ctr": {
    "shape": "square",
    "half_extent_nm": 10.0
  },
  "active_runway": "18",
  "runways": [
    {
      "runway_id": "18_36",
      "surface": "asphalt",
      "length_ft": 4000,
      "ends": ["18", "36"]
    }
  ],
  "sids": [
    { "sid_id": "north1a", "family": "north1", "runway": "18", "initial_heading_deg": 360, "paired_sid_id": "north1b" },
    { "sid_id": "east1a", "family": "east1", "runway": "18", "initial_heading_deg": 90, "paired_sid_id": "east1b" },
    { "sid_id": "south1a", "family": "south1", "runway": "18", "initial_heading_deg": 180, "paired_sid_id": "south1b" },
    { "sid_id": "west1a", "family": "west1", "runway": "18", "initial_heading_deg": 270, "paired_sid_id": "west1b" },
    { "sid_id": "north1b", "family": "north1", "runway": "36", "initial_heading_deg": 360, "paired_sid_id": "north1a" },
    { "sid_id": "east1b", "family": "east1", "runway": "36", "initial_heading_deg": 90, "paired_sid_id": "east1a" },
    { "sid_id": "south1b", "family": "south1", "runway": "36", "initial_heading_deg": 180, "paired_sid_id": "south1a" },
    { "sid_id": "west1b", "family": "west1", "runway": "36", "initial_heading_deg": 270, "paired_sid_id": "west1a" }
  ],
  "ils": [
    { "runway": "18", "course_deg": 176, "visible_when_active": true },
    { "runway": "36", "course_deg": 356, "visible_when_active": true }
  ]
}
```

## WebSocket Connection

Endpoint:

- `GET /api/v1/ws?token=<token>&session_id=<session_id>`

Either `trainer_token` or `student_token` may be used.

Handshake validation:

- token must be valid
- token must be authorized for the session
- student token must match the bound student position

After connect:

1. server sends `hello`
2. server sends full `session_state`
3. client sends commands
4. server broadcasts accepted state changes

## WebSocket Envelope

All WebSocket messages use a common envelope.

```json
{
  "type": "trainer_command",
  "message_id": "msg_123",
  "session_id": "sess_01",
  "sent_at": "2026-04-17T15:00:00Z",
  "payload": {}
}
```

Fields:

- `type`: message discriminator
- `message_id`: client-generated or server-generated unique message ID
- `session_id`: target session
- `sent_at`: sender timestamp
- `payload`: type-specific JSON body

## Server To Client Events

### `hello`

Sent immediately after successful WebSocket upgrade.

```json
{
  "type": "hello",
  "message_id": "srv_1",
  "session_id": "sess_01",
  "sent_at": "2026-04-17T15:00:00Z",
  "payload": {
    "connection_id": "conn_123",
    "role": "trainer"
  }
}
```

### `session_state`

Full canonical snapshot.

Payload:

- `SessionState`

Use cases:

- initial sync
- recovery after reconnect
- periodic resync if needed

### `session_updated`

Session metadata changed.

```json
{
  "status": "running",
  "active_runway": "36",
  "revision": 43
}
```

If active runway changes, the server may follow this event with `aircraft_updated` events for auto-reassigned aircraft.

### `aircraft_created`

```json
{
  "aircraft": {
    "aircraft_id": "ac_01",
    "callsign": "ABC123",
    "departure_airfield": "AAAA",
    "destination_airfield": "AAAN",
    "category": "vfr_c172",
    "aircraft_type": "C172",
    "flight_rules": "vfr",
    "route": "VFR_PATTERN",
    "status": "new",
    "movement_mode": "parked",
    "controlled_by_position": "gnd",
    "assigned_runway": null,
    "assigned_sid": null,
    "assigned_squawk": null,
    "assigned_altitude_ft": null,
    "current_node": "apron",
    "target_node": null,
    "next_waypoint": null,
    "ground_speed_kt": 0,
    "air_speed_kt": null,
    "altitude_ft": 0,
    "x_nm": -0.5,
    "y_nm": -0.7,
    "trainer_profile": null,
    "revision": 1
  }
}
```

### `aircraft_updated`

```json
{
  "aircraft": {
    "aircraft_id": "ac_01",
    "status": "taxi",
    "movement_mode": "taxiing",
    "current_node": "twy_main",
    "target_node": "hold_18",
    "ground_speed_kt": 15,
    "revision": 8
  }
}
```

This event may send:

- full aircraft object
- or partial patch object

Recommendation for v1:

- send full `AircraftState` to simplify client code

### `aircraft_removed`

```json
{
  "aircraft_id": "ac_01",
  "revision": 9
}
```

### `command_rejected`

```json
{
  "correlation_id": "msg_123",
  "code": "invalid_status_transition",
  "message": "Aircraft cannot transition from pushback to lineup"
}
```

### `system_notice`

```json
{
  "code": "student_disconnected",
  "message": "Student connection lost"
}
```

Suggested additional notices:

```json
{
  "code": "trainer_cooldown_started",
  "message": "All trainers disconnected. Session will close in 3 minutes if no trainer reconnects."
}
```

```json
{
  "code": "trainer_reconnected",
  "message": "Trainer reconnected before cooldown expiry."
}
```

```json
{
  "code": "session_log_written",
  "message": "Session action log is being written to file."
}
```

## Client To Server Events

### `ping`

```json
{
  "nonce": "abc"
}
```

### `trainer_command`

Single entrypoint for trainer actions.

Payload:

```json
{
  "command": {
    "kind": "spawn_aircraft",
    "arguments": {}
  }
}
```

Suggested trainer command kinds:

- `spawn_aircraft`
- `remove_aircraft`
- `set_active_runway`
- `assign_runway`
- `set_speed`
- `set_path`
- `launch_path`
- `stop_ground_aircraft`
- `set_aircraft_profile`
- `spawn_aircraft`
- `remove_aircraft`
- `set_active_runway`
- `assign_runway_in_work`
- `set_speed`
- `set_path`
- `launch_path`
- `stop_ground_aircraft`
- `trigger_go_around`
- `trigger_rejected_takeoff`
- `pause_session`
- `resume_session`

#### `spawn_aircraft`

This command is for manually adding an extra aircraft during a live session.
It is template-based in v1.

```json
{
  "command": {
    "kind": "spawn_aircraft",
    "arguments": {
      "template": "a320_ifr",
      "callsign": "ABC123",
      "squawk_mode": "standby",
      "initial_state": "inbound",
      "spawn_point": {
        "x_nm": 8.2,
        "y_nm": 4.6
      },
      "initial_path": {
        "path_kind": "radar_points",
        "points": [
          {
            "seq": 1,
            "x_nm": 4.0,
            "y_nm": 2.0,
            "target_speed_kt": 180,
            "target_altitude": {
              "mode": "msl_ft",
              "value_ft": 2500
            }
          },
          {
            "seq": 2,
            "x_nm": 1.5,
            "y_nm": 0.5,
            "target_speed_kt": 140,
            "target_altitude": {
              "mode": "msl_ft",
              "value_ft": 1500
            }
          }
        ]
      }
    }
  }
}
```

Allowed templates:

- `c172_vfr`
- `a320_ifr`

Notes:

- trainer chooses template, callsign, and squawk mode
- trainer chooses the initial radar position freely
- trainer assigns the initial aircraft state
- if the aircraft is created airborne, trainer must also provide an initial path
- manual creation does not snap the aircraft to predefined positions
- created aircraft become visible to all connected clients immediately
- student later assigns squawk code through the traffic manager
- predefined scenario aircraft do not require this command

#### `assign_runway`

Trainer may assign or change the runway in work for an aircraft.

```json
{
  "command": {
    "kind": "assign_runway",
    "arguments": {
      "aircraft_id": "ac_01",
      "assigned_runway": "18"
    }
  }
}
```

#### `set_active_runway`

Trainer may switch the active runway for the whole session at any time.

```json
{
  "command": {
    "kind": "set_active_runway",
    "arguments": {
      "active_runway": "36"
    }
  }
}
```

Behavior:

- updates session active runway
- updates which ILS is rendered as active
- auto-reassigns all ground aircraft to the new runway in work, even if the aircraft had no prior runway assignment
- auto-converts SID only when the aircraft already has a SID assigned and is still on the ground

#### `move_to_node`

```json
{
  "command": {
    "kind": "move_to_node",
    "arguments": {
      "aircraft_id": "ac_01",
      "target_node": "hold_18"
    }
  }
}
```

#### `set_speed`

Used when the trainer wants direct control over aircraft speed.

```json
{
  "command": {
    "kind": "set_speed",
    "arguments": {
      "aircraft_id": "ac_01",
      "target_speed_kt": 210
    }
  }
}
```

#### `set_path`

Used when the trainer wants to define an explicit path for aircraft movement by marking radar points.

```json
{
  "command": {
    "kind": "set_path",
    "arguments": {
      "aircraft_id": "ac_01",
      "path_kind": "radar_points",
      "points": [
        {
          "seq": 1,
          "x_nm": -0.4,
          "y_nm": -0.7,
          "target_speed_kt": 15,
          "target_altitude": {
            "mode": "gnd"
          }
        },
        {
          "seq": 2,
          "x_nm": 0.2,
          "y_nm": 1.8,
          "target_speed_kt": 90,
          "target_altitude": {
            "mode": "msl_ft",
            "value_ft": 1500
          }
        }
      ]
    }
  }
}
```

Suggested `path_kind` values:

- `radar_points`

Rules:

- trainer places path points directly on the sector display
- trainer first selects the aircraft that the draft path belongs to
- trainer places all points first and edits point parameters afterward
- the drafted path and ordered points must be visible before launch
- trainer may undo only the last drafted point before launch
- trainer uses explicit `Finish pathing` before point-parameter editing begins
- after `Finish pathing`, point geometry is fixed for that draft
- after `Finish pathing`, only point speed and altitude / `GND` remain editable
- each point defines the target speed for that segment
- each point defines either:
  - `mode = gnd`
  - or `mode = msl_ft` with an altitude value
- `gnd` means the aircraft should remain ground-bound for that segment
- the full path is drafted first and does not begin moving the aircraft by itself
- launch should not impose protective trainer-side path validation beyond basic payload decoding in v1
- once the path is launched, it may not be edited in place
- trainer must stop the aircraft before replacing the active path
- the server follows the submitted path as the authoritative movement plan

#### `launch_path`

Used when the trainer wants the aircraft to start following the already drafted path.

```json
{
  "command": {
    "kind": "launch_path",
    "arguments": {
      "aircraft_id": "ac_01"
    }
  }
}
```

#### `stop_ground_aircraft`

Used when the trainer wants to stop an aircraft that is still on the ground.

```json
{
  "command": {
    "kind": "stop_ground_aircraft",
    "arguments": {
      "aircraft_id": "ac_01"
    }
  }
}
```

#### `trigger_go_around`

Used when the trainer wants an arrival to break off the approach.

```json
{
  "command": {
    "kind": "trigger_go_around",
    "arguments": {
      "aircraft_id": "ac_01"
    }
  }
}
```

Rules:

- the aircraft follows runway heading only as the immediate go-around behavior
- after that, the trainer must manually define and launch the next path

#### `trigger_rejected_takeoff`

Used when the trainer wants to abort a departure while the aircraft is on the runway.

```json
{
  "command": {
    "kind": "trigger_rejected_takeoff",
    "arguments": {
      "aircraft_id": "ac_01"
    }
  }
}
```

Rules:

- available only when the aircraft is in an on-runway departure state
- may be implemented as a dedicated trainer action even if some stop behavior is also represented in the path model

#### `set_aircraft_profile`

Used to assign trainer-controlled movement behavior.

```json
{
  "command": {
    "kind": "set_aircraft_profile",
    "arguments": {
      "aircraft_id": "ac_01",
      "profile": "standard_departure_ifr_a320"
    }
  }
}
```

Suggested v1 profiles:

- `standard_departure_vfr_c172`
- `standard_departure_ifr_a320`
- `standard_arrival_ifr_a320`
- `vfr_pattern_c172`

### `student_update_aircraft`

Single entrypoint for student actions on an aircraft.

```json
{
  "aircraft_id": "ac_01",
  "changes": {
    "status": "clearance_given",
    "assigned_runway": "18",
    "assigned_sid": "east1a",
    "assigned_squawk": "4123",
    "assigned_altitude_ft": 5000
  }
}
```

Validation rules:

- student may only update assumed aircraft relevant to the session position
- trainer may update any aircraft
- `GND` and `TWR` may use the same status set, but operational use still depends on session role and training context
- both trainer and student may assign runway in work

### `student_handoff_aircraft`

Explicitly marks that the student is handing the aircraft off to an abstract next position in the training flow.

```json
{
  "aircraft_id": "ac_01",
  "handoff_target": "next_position"
}
```

Notes:

- the handoff target is abstract rather than a real downstream ATC unit
- this is a training action, not a real network coordination action
- the client should display that the aircraft has been handed off
- `assigned_sid` must match assigned runway
- `assigned_squawk` should use a simple four-digit training format
- `assigned_altitude_ft` must follow simple format and range validation
- status transition must be valid
- student must not directly change movement state, speed, path, altitude, or position

## Scenario Definitions

Scenarios define the initial aircraft arrangement for a session.

Scenario definition responsibilities:

- place predefined aircraft in airport or airspace locations
- define initial callsign, template/category, route context, and status
- define whether an aircraft starts on stand, in CTR, or in the VFR pattern
- use fixed named placements rather than arbitrary coordinates in v1

There is no built-in scenario editor.
For v1, scenarios should be loadable from external files.
The application should also include exactly one built-in standard scenario so the system remains usable without external scenario files.

Recommended v1 scenario file format:

- use `TOML`
- store one scenario per file
- include:
  - `scenario_id`
  - `name`
  - `student_position_type`
  - `initial_active_runway`
  - `metar`
  - `aircraft[]`
- each aircraft entry should include:
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

Example scenario file:

- [scenario-gnd-36-medium.toml](/Users/sasha/dev/atc-trainer/docs/scenario-gnd-36-medium.toml)

### `student_assume_aircraft`

Explicitly marks that the student is taking responsibility for an aircraft in the traffic manager.

```json
{
  "aircraft_id": "ac_01"
}
```

This helps the UI display ownership and the trainer observe student workflow.

Focus and assumption rules in v1:

- focus is single-selection UI state
- assumption is separate operational state
- changing focus does not release aircraft assumption
- a student may assume multiple aircraft while only one is focused

## Status Transition Rules

The server must validate transitions.

### Departure Path

Recommended v1 status path:

- `new -> cleared`
- `cleared -> push`
- `push -> startup`
- `startup -> taxi`
- `taxi -> on_runway`
- `on_runway -> airborne`

### Notes

- the trainer may change movement without the student changing status
- movement mode and student status are related but not identical
- the server should permit trainer override actions with explicit logging
- the server should persist accepted actions to a session log file
- airborne context should be represented through assigned SID / route / next waypoint fields

## Control Ownership Rules

Ownership in v1 is position-based.

- departure-ground aircraft default to `gnd`
- airborne departure aircraft may transfer to `twr`
- inbound arrival aircraft default to `twr`
- after landing and runway vacate, control may transfer depending on exercise design

This should be encoded as:

- `controlled_by_position`

The client should use this field to decide whether the student can edit an aircraft.

## Visual Rules

Client display rules locked for v1:

- all aircraft are always visible on the sector
- airborne aircraft show a one-minute forward vector only
- unassumed traffic labels show callsign only
- assumed and focused traffic labels show:
  - callsign
  - aircraft type
  - next waypoint
  - current altitude
  - assigned altitude
- label frame color depends on squawk mode:
  - `off` -> gray
  - `standby` -> white
  - `charlie` -> green

Additional label rules:

- for SID-driven IFR traffic, the displayed next waypoint should be the directional outer fix name
- example: `north1a` and `north1b` both display next waypoint `NORTH`

Built-in sector navigation points:

- the sector includes named outer points `NORTH`, `EAST`, `SOUTH`, and `WEST`
- each point is located `10 NM` beyond the CTR boundary in its respective direction

Assigned runway auto-clear rules:

- when operated in `GND`, `assigned_runway` is cleared automatically when the aircraft is assumed
- when operated in `TWR`, `assigned_runway` is cleared automatically when the aircraft has vacated the runway

## Logging

The server must write human-readable session logs to files.

Minimum v1 log requirements:

- one line per accepted action or state change
- include timestamp
- include actor role / identity when available
- include aircraft identifier when applicable
- include concise before / after state information

The log format should remain replay-friendly for v2 even if replay UI is not implemented in v1.

Recommended v1 log line style:

```text
2026-04-17T18:42:11Z session=s_01 actor=trainer:t_02 aircraft=ac_07 action=set_path result="3 points, launch pending"
2026-04-17T18:42:19Z session=s_01 actor=student:position_gnd aircraft=ac_07 action=update result="status=taxi runway=18 sid=east1a sqwk=4123 alt=A050"
2026-04-17T18:42:31Z session=s_01 actor=trainer:t_01 aircraft=ac_07 action=launch_path result="movement started"
```

## Voice Communication Model

Voice is out of scope for the software in v1.

- the application does not provide built-in voice chat
- trainer and student are expected to use the same external voice channel
- the software supports the exercise by synchronizing labels, traffic state, and aircraft movement

## Logging And Replay

For v1:

- the server writes session action logs to files
- logs include timestamps, actor identity, command type, and accepted state changes
- logs are intended for inspection and future replay support

For v2:

- the system should support replaying a recorded session from stored logs
- replay should reconstruct trainer actions, student actions, and aircraft movement in time order

## Weather Handling

For v1:

- there is no dynamic weather simulation
- trainer loads or enters a METAR before session start
- the METAR is stored as session briefing data
- runway selection and traffic behavior remain explicit trainer decisions

## Airborne Speed Profiles

The protocol should not send raw physics commands for v1.
It should send named profiles and state snapshots.

Recommended default speed guidance:

- `C172` VFR departure climb: `75-85 KIAS`
- `C172` local pattern: about `90 KIAS`
- `C172` local cruise/training area: about `105-120 KTAS`
- `A320` IFR below `10,000 ft`: `250 KIAS`
- `A320` IFR above `10,000 ft`: about `300 KIAS`

The server may internally use profile tables such as:

```json
{
  "profile_id": "standard_departure_ifr_a320",
  "phases": [
    { "phase": "takeoff_roll", "target_speed_kt": 140 },
    { "phase": "initial_climb", "target_speed_kt": 210 },
    { "phase": "below_10000", "target_speed_kt": 250 },
    { "phase": "above_10000", "target_speed_kt": 300 }
  ]
}
```

## Error Model

Suggested error codes:

- `unauthorized`
- `forbidden`
- `invalid_session_hash`
- `session_not_found`
- `session_not_running`
- `student_position_missing`
- `student_position_occupied`
- `invalid_aircraft_id`
- `invalid_status_transition`
- `invalid_runway`
- `invalid_sid`
- `invalid_squawk`
- `aircraft_not_controllable_by_position`
- `command_conflict`
- `revision_conflict`

HTTP error shape:

```json
{
  "error": {
    "code": "invalid_sid",
    "message": "Assigned SID does not match runway 36"
  }
}
```

## Revision Strategy

Each mutable object should include a revision number.

At minimum:

- session revision
- aircraft revision

This supports:

- client reconciliation
- stale write rejection
- easier debugging

Optional v1 enhancement:

- allow commands to include `expected_revision`

## Recommended Rust Type Layout

Suggested modules inside `atc-shared`:

```text
protocol/
  auth.rs
  session.rs
  aircraft.rs
  sector.rs
  websocket.rs
  error.rs
```

Suggested naming:

- `TrainerLoginRequest`
- `TrainerLoginResponse`
- `CreateSessionRequest`
- `CreateStudentPositionRequest`
- `JoinStudentRequest`
- `JoinStudentResponse`
- `SessionState`
- `AircraftState`
- `WsEnvelope<T>`
- `ClientEvent`
- `ServerEvent`

## Recommended Implementation Sequence

1. Define shared enums and IDs in `atc-shared`.
2. Implement HTTP auth and session creation in `atc-server`.
3. Implement WebSocket handshake and `session_state` broadcast.
4. Implement `trainer_command.spawn_aircraft`.
5. Implement `student_update_aircraft` validation and broadcast.
6. Implement movement profiles and route-node transitions.

## Open Protocol Decisions

These are narrower implementation details, not product-level open questions:

- Should trainer login always return `lead_trainer`, or can additional trainers join as `observer_trainer` through a separate invite flow?
- Should WebSocket `aircraft_updated` send full objects only, or support partial patch payloads later?
- Should `TWR` be allowed to set departure runway/SID for aircraft already transferred from `GND`, or should those fields remain immutable after handoff?
