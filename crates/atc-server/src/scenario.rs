use atc_shared::aircraft::*;
use atc_shared::ids::AircraftId;
use atc_shared::scenario::ScenarioFile;

/// Convert a scenario aircraft definition into an initial runtime `AircraftState`.
pub fn scenario_aircraft_to_state(
    sa: &atc_shared::scenario::ScenarioAircraft,
) -> AircraftState {
    let controlled_by = match sa.initial_status {
        AircraftStatus::Airborne => ControlledByPosition::Twr,
        _ => ControlledByPosition::Gnd,
    };
    let aircraft_type = match sa.template {
        AircraftCategory::VfrC172 => "C172",
        AircraftCategory::IfrA320 => "A320",
    }
    .to_string();

    AircraftState {
        aircraft_id: AircraftId::new(sa.aircraft_id.clone()),
        origin: AircraftOrigin::Scenario,
        callsign: sa.callsign.clone(),
        departure_airfield: sa.departure_airfield.clone(),
        destination_airfield: sa.destination_airfield.clone(),
        category: sa.template,
        aircraft_type,
        flight_rules: sa.flight_rules,
        route: sa.route.clone(),
        status: sa.initial_status,
        movement_mode: status_to_initial_movement(sa.initial_status),
        controlled_by_position: controlled_by,
        squawk_mode: sa.squawk_mode,
        assigned_runway: sa.assigned_runway.clone(),
        assigned_sid: sa.assigned_sid.clone(),
        assigned_squawk: None,
        assigned_altitude_ft: sa.assigned_altitude_ft,
        current_node: Some(sa.initial_spawn.clone()),
        target_node: None,
        scenario_placement: Some(sa.initial_spawn.clone()),
        next_waypoint: None,
        ground_speed_kt: 0.0,
        air_speed_kt: None,
        altitude_ft: 0.0,
        x_nm: 0.0,
        y_nm: 0.0,
        trainer_profile: None,
        revision: 1,
    }
}

fn status_to_initial_movement(s: AircraftStatus) -> MovementMode {
    match s {
        AircraftStatus::New | AircraftStatus::Cleared => MovementMode::Parked,
        AircraftStatus::Push => MovementMode::Pushback,
        AircraftStatus::Startup => MovementMode::Parked,
        AircraftStatus::Taxi => MovementMode::Taxiing,
        AircraftStatus::OnRunway => MovementMode::Holding,
        AircraftStatus::Airborne => MovementMode::Airborne,
    }
}

pub fn initial_aircraft(scenario: &ScenarioFile) -> Vec<AircraftState> {
    scenario
        .aircraft
        .iter()
        .map(scenario_aircraft_to_state)
        .collect()
}
