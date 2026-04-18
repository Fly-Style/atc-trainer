use atc_shared::aircraft::*;
use atc_shared::ids::AircraftId;
use atc_shared::scenario::ScenarioFile;
use atc_shared::validation::sid_outer_fix;

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
        next_waypoint: sa.assigned_sid.as_deref().and_then(sid_outer_fix).map(str::to_string),
        ground_speed_kt: 0.0,
        air_speed_kt: None,
        altitude_ft: 0.0,
        x_nm: 0.0,
        y_nm: 0.0,
        trainer_profile: None,
        assumed_by_student: false,
        handed_off: false,
        draft_path: None,
        active_path: None,
        path_run_id: 0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use atc_shared::scenario::ScenarioFile;

    #[test]
    fn sid_sets_directional_next_waypoint() {
        let scenario =
            ScenarioFile::from_toml_str(include_str!("../../../docs/scenario-gnd-36-medium.toml"))
                .expect("scenario parses");
        let aircraft = initial_aircraft(&scenario);
        let northbound = aircraft
            .iter()
            .find(|aircraft| aircraft.callsign == "BTI201")
            .expect("northbound aircraft");
        assert_eq!(northbound.assigned_sid.as_deref(), Some("north1b"));
        assert_eq!(northbound.next_waypoint.as_deref(), Some("NORTH"));
    }
}
