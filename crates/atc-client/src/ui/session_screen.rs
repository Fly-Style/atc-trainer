//! SessionScreen: top status strip + large sector + bottom traffic manager
//! and a trainer-only toolbox docked on the right.

use crate::core::command::AppCommand;
use crate::core::state::AuthState;
use crate::ui::app::{AtcApp, Message, SessionField, TrainerField};
use crate::ui::sector_canvas::SectorCanvas;
use atc_shared::role::Role;
use atc_shared::session::SessionStatus;
use iced::widget::{
    button, canvas, column, container, row, scrollable, text, text_input, Column, Row, Space,
};
use iced::{Alignment, Element, Length};

pub fn view(app: &AtcApp) -> Element<'_, Message> {
    let top = top_strip(app);
    let sector = container(
        canvas(SectorCanvas::new(app))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);
    let bottom = traffic_manager(app);

    let main_column = column![sector, bottom]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(8);

    let content = match app.state.auth.role() {
        Some(Role::Trainer) => row![
            container(main_column).width(Length::FillPortion(4)),
            container(trainer_toolbox(app))
                .width(Length::FillPortion(1))
                .padding(8),
        ]
        .height(Length::Fill),
        _ => row![container(main_column).width(Length::Fill)].height(Length::Fill),
    };

    column![top, content].height(Length::Fill).spacing(8).into()
}

fn top_strip(app: &AtcApp) -> Element<'_, Message> {
    let session = &app.state.session.as_ref().expect("session required").state;
    let role = match app.state.auth.role() {
        Some(Role::Trainer) => "TRAINER",
        Some(Role::Student) => "STUDENT",
        None => "—",
    };
    let position = session
        .student_position
        .as_ref()
        .map(|position| format!("{:?}", position.position_type).to_uppercase())
        .unwrap_or_else(|| "—".into());
    let status = match session.status {
        SessionStatus::Draft => "draft",
        SessionStatus::WaitingForStudent => "waiting_for_student",
        SessionStatus::Running => "running",
        SessionStatus::Paused => "paused",
        SessionStatus::Ended => "ended",
    };

    let mut strip = row![
        text(&session.name).size(18),
        text(format!("[{}]", status)).size(14),
        text(format!("RWY {}", session.active_runway)).size(14),
        text(format!("POS {}", position)).size(14),
        Space::with_width(Length::Fill),
        text(format!("{}  rev #{}", role, session.revision)).size(12),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    if matches!(app.state.auth, AuthState::Trainer { .. }) {
        if session.status != SessionStatus::Running {
            strip = strip.push(button("Start").on_press(Message::Command(AppCommand::StartSession {
                session_id: session.session_id.clone(),
            })));
        }
        strip = strip
            .push(button("Pause").on_press(Message::Command(AppCommand::PauseSession)))
            .push(button("Resume").on_press(Message::Command(AppCommand::ResumeSession)));
    }

    container(strip)
        .padding(8)
        .style(|_| container::Style::default().background(iced::Background::Color(iced::color!(0x20242D))))
        .into()
}

fn traffic_manager(app: &AtcApp) -> Element<'_, Message> {
    let Some(aircraft) = app.focused_aircraft() else {
        return container(text("Traffic manager: no focused aircraft").size(14))
            .padding(12)
            .height(Length::Fixed(180.0))
            .style(panel_style)
            .into();
    };

    let title = row![
        text(format!("{} / {}", aircraft.callsign, aircraft.aircraft_type)).size(18),
        Space::with_width(Length::Fill),
        text(format!(
            "{}  {} -> {}  next {}",
            aircraft.route,
            aircraft.departure_airfield,
            aircraft.destination_airfield,
            aircraft.next_waypoint.clone().unwrap_or_else(|| "-".into())
        ))
        .size(12),
    ];

    let read_row = row![
        summary_chip("Rules", format!("{:?}", aircraft.flight_rules).to_uppercase()),
        summary_chip("Status", format!("{:?}", aircraft.status).to_lowercase()),
        summary_chip("Current ALT", format!("{:.0}", aircraft.altitude_ft)),
        summary_chip(
            "Assigned ALT",
            aircraft
                .assigned_altitude_ft
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".into()),
        ),
        summary_chip("Assumed", if aircraft.assumed_by_student { "yes" } else { "no" }),
        summary_chip("Handoff", if aircraft.handed_off { "yes" } else { "no" }),
    ]
    .spacing(8);

    let fields = row![
        labelled_input(
            "Runway",
            &app.session_form.assigned_runway,
            SessionField::AssignedRunway
        ),
        labelled_input("SID", &app.session_form.assigned_sid, SessionField::AssignedSid),
        labelled_input(
            "SQWK",
            &app.session_form.assigned_squawk,
            SessionField::AssignedSquawk
        ),
        labelled_input(
            "Altitude",
            &app.session_form.assigned_altitude,
            SessionField::AssignedAltitude
        ),
        labelled_input("Status", &app.session_form.status, SessionField::Status),
    ]
    .spacing(8);

    let mut actions = Row::new().spacing(8);
    match app.state.auth.role() {
        Some(Role::Student) => {
            actions = actions
                .push(button("Assume").on_press(Message::Command(AppCommand::AssumeAircraft {
                    aircraft_id: aircraft.aircraft_id.clone(),
                })))
                .push(button("Apply").on_press(Message::Command(AppCommand::UpdateAircraft {
                    aircraft_id: aircraft.aircraft_id.clone(),
                    changes: app.build_student_changes(),
                })))
                .push(button("Handoff").on_press(Message::Command(AppCommand::HandoffAircraft {
                    aircraft_id: aircraft.aircraft_id.clone(),
                })));
        }
        Some(Role::Trainer) => {
            actions = actions
                .push(button("Assume").on_press(Message::Command(AppCommand::AssumeAircraft {
                    aircraft_id: aircraft.aircraft_id.clone(),
                })))
                .push(button("Apply").on_press(Message::Command(AppCommand::UpdateAircraft {
                    aircraft_id: aircraft.aircraft_id.clone(),
                    changes: app.build_student_changes(),
                })))
                .push(button("Handoff").on_press(Message::Command(AppCommand::HandoffAircraft {
                    aircraft_id: aircraft.aircraft_id.clone(),
                })))
                .push(button("Set runway").on_press(Message::Command(
                    AppCommand::AssignRunwayInWork {
                        aircraft_id: aircraft.aircraft_id.clone(),
                        runway: optional_string(&app.session_form.assigned_runway),
                    },
                )));
        }
        None => {}
    }

    container(column![title, read_row, fields, actions].spacing(10))
        .padding(12)
        .height(Length::Fixed(180.0))
        .style(panel_style)
        .into()
}

fn trainer_toolbox(app: &AtcApp) -> Element<'_, Message> {
    let focused_id = app.focused_aircraft().map(|aircraft| aircraft.aircraft_id.clone());
    let mut column = Column::new().spacing(10).push(text("Trainer Tools").size(18));

    column = column
        .push(text("Spawn aircraft").size(14))
        .push(trainer_input("Callsign", &app.trainer_form.callsign, TrainerField::Callsign))
        .push(row![
            trainer_input("Template", &app.trainer_form.template, TrainerField::Template),
            trainer_input("Squawk", &app.trainer_form.squawk_mode, TrainerField::SquawkMode),
        ]
        .spacing(6))
        .push(row![
            trainer_input("State", &app.trainer_form.initial_state, TrainerField::InitialState),
            trainer_input("RWY", &app.trainer_form.runway, TrainerField::Runway),
        ]
        .spacing(6))
        .push(row![
            trainer_input("X", &app.trainer_form.spawn_x, TrainerField::SpawnX),
            trainer_input("Y", &app.trainer_form.spawn_y, TrainerField::SpawnY),
        ]
        .spacing(6))
        .push(
            button("Create aircraft").on_press(Message::Command(AppCommand::SpawnAircraft {
                template: app.build_spawn_template(),
                callsign: app.trainer_form.callsign.clone(),
                squawk_mode: app.build_spawn_squawk_mode(),
                initial_x_nm: app.trainer_form.spawn_x.parse().unwrap_or_default(),
                initial_y_nm: app.trainer_form.spawn_y.parse().unwrap_or_default(),
                initial_state: app.build_initial_state(),
                initial_path: if app.trainer_form.draft_points.is_empty() {
                    None
                } else {
                    Some(app.build_draft_path())
                },
            })),
        );

    column = column.push(text("Focused aircraft").size(14));
    if let Some(aircraft_id) = focused_id {
        column = column
            .push(trainer_input("Speed", &app.trainer_form.speed, TrainerField::Speed))
            .push(
                row![
                    button("Set speed").on_press(Message::Command(AppCommand::SetSpeed {
                        aircraft_id: aircraft_id.clone(),
                        target_speed_kt: app.trainer_form.speed.parse().unwrap_or_default(),
                    })),
                    button("Remove").on_press(Message::Command(AppCommand::RemoveAircraft {
                        aircraft_id: aircraft_id.clone(),
                    })),
                ]
                .spacing(6),
            )
            .push(
                row![
                    button("Active RWY 18")
                        .on_press(Message::Command(AppCommand::SetActiveRunway { runway: "18".into() })),
                    button("Active RWY 36")
                        .on_press(Message::Command(AppCommand::SetActiveRunway { runway: "36".into() })),
                ]
                .spacing(6),
            )
            .push(text("Draft path").size(14))
            .push(row![
                trainer_input("PX", &app.trainer_form.point_x, TrainerField::PointX),
                trainer_input("PY", &app.trainer_form.point_y, TrainerField::PointY),
            ]
            .spacing(6))
            .push(row![
                trainer_input("PSpd", &app.trainer_form.point_speed, TrainerField::PointSpeed),
                trainer_input("PAlt/GND", &app.trainer_form.point_altitude, TrainerField::PointAltitude),
            ]
            .spacing(6))
            .push(
                row![
                    button("Add point").on_press(Message::AddDraftPoint),
                    button("Undo").on_press(Message::UndoDraftPoint),
                    button("Finish pathing").on_press(Message::FinishPathing),
                ]
                .spacing(6),
            )
            .push(path_points_list(app))
            .push(
                row![
                    button("Set path").on_press(Message::Command(AppCommand::SetPath {
                        aircraft_id: aircraft_id.clone(),
                        path: app.build_draft_path(),
                    })),
                    button("Launch").on_press(Message::Command(AppCommand::LaunchPath {
                        aircraft_id: aircraft_id.clone(),
                    })),
                ]
                .spacing(6),
            )
            .push(
                row![
                    button("Stop").on_press(Message::Command(AppCommand::StopGroundAircraft {
                        aircraft_id: aircraft_id.clone(),
                    })),
                    button("Go around").on_press(Message::Command(AppCommand::TriggerGoAround {
                        aircraft_id: aircraft_id.clone(),
                    })),
                    button("RTO").on_press(Message::Command(AppCommand::TriggerRejectedTakeoff {
                        aircraft_id,
                    })),
                ]
                .spacing(6),
            );
    } else {
        column = column.push(text("No focused aircraft").size(12));
    }

    if let Some(error) = &app.state.last_error {
        column = column.push(text(format!("error: {error}")).size(12).color(iced::color!(0xCC5555)));
    }

    scrollable(container(column).style(panel_style).padding(10)).into()
}

fn path_points_list(app: &AtcApp) -> Element<'_, Message> {
    let mut points = Column::new().spacing(4);
    for point in &app.trainer_form.draft_points {
        let altitude = match point.target_altitude {
            atc_shared::aircraft::TargetAltitude::Gnd => "GND".to_string(),
            atc_shared::aircraft::TargetAltitude::MslFt { value_ft } => value_ft.to_string(),
        };
        points = points.push(text(format!(
            "#{}  x={:.1} y={:.1}  spd={:.0} alt={}",
            point.seq, point.x_nm, point.y_nm, point.target_speed_kt, altitude
        )));
    }
    points.into()
}

fn labelled_input<'a>(label: &'a str, value: &'a str, field: SessionField) -> Element<'a, Message> {
    column![
        text(label).size(12),
        text_input(label, value)
            .on_input(move |value| Message::SessionFieldChanged(field, value))
            .padding(6),
    ]
    .width(Length::Fill)
    .into()
}

fn trainer_input<'a>(label: &'a str, value: &'a str, field: TrainerField) -> Element<'a, Message> {
    column![
        text(label).size(12),
        text_input(label, value)
            .on_input(move |value| Message::TrainerFieldChanged(field, value))
            .padding(6),
    ]
    .width(Length::Fill)
    .into()
}

fn summary_chip(label: &'static str, value: impl Into<String>) -> Element<'static, Message> {
    let value = value.into();
    container(column![text(label).size(11), text(value).size(13)].spacing(2))
        .padding(6)
        .style(panel_style)
        .into()
}

fn panel_style(_: &iced::Theme) -> container::Style {
    container::Style::default()
        .background(iced::Background::Color(iced::color!(0x181C24)))
        .border(iced::border::rounded(4).width(1).color(iced::color!(0x2D3440)))
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
