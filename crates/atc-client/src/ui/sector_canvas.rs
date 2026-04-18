//! Sector canvas: airport, CTR, ILS, aircraft markers + labels, with local
//! zoom/pan and focus hit-testing. Mouse wheel zooms, left-drag pans, click
//! focuses an aircraft. Pure rendering pulled from `core::state::ViewState`.

use crate::core::command::AppCommand;
use crate::ui::app::{AtcApp, Message};
use atc_shared::aircraft::{AircraftState, SquawkMode, TargetAltitude};
use atc_shared::role::Role;
use atc_shared::sector::{CtrShape, NamedPoint, SectorMetadata, WorldPoint};
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text};
use iced::{mouse, Color, Point, Rectangle, Renderer, Theme};

pub struct SectorCanvas<'a> {
    app: &'a AtcApp,
    cache: Cache,
}

impl<'a> SectorCanvas<'a> {
    pub fn new(app: &'a AtcApp) -> Self {
        Self {
            app,
            cache: Cache::new(),
        }
    }
}

#[derive(Default)]
pub struct CanvasInteraction {
    pub dragging: Option<Point>,
}

impl<'a> canvas::Program<Message> for SectorCanvas<'a> {
    type State = CanvasInteraction;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        let Some(pos) = cursor.position_in(bounds) else {
            return (canvas::event::Status::Ignored, None);
        };
        let canvas_size = (bounds.width, bounds.height);

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let hit = self.app.state.hit_test_aircraft(
                    (pos.x, pos.y),
                    canvas_size,
                    8.0,
                );
                if let Some(id) = hit {
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::Command(AppCommand::FocusAircraft(id))),
                    );
                }
                if matches!(self.app.state.auth.role(), Some(Role::Trainer))
                    && self.app.state.view.focused_aircraft.is_some()
                    && self.app.trainer_form.draft_mode_active
                    && !self.app.trainer_form.path_geometry_finished
                {
                    let (x_nm, y_nm) = self
                        .app
                        .state
                        .view
                        .screen_to_world((pos.x, pos.y), canvas_size);
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::CanvasDraftPointAdded { x_nm, y_nm }),
                    );
                }
                state.dragging = Some(pos);
                (canvas::event::Status::Captured, None)
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(prev) = state.dragging {
                    let dx_px = pos.x - prev.x;
                    let dy_px = pos.y - prev.y;
                    state.dragging = Some(pos);
                    let zoom = self.app.state.view.zoom_px_per_nm.max(1.0);
                    let dx_nm = -dx_px / zoom;
                    let dy_nm = dy_px / zoom; // screen y down → world y up
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::Command(AppCommand::PanByNm { dx: dx_nm, dy: dy_nm })),
                    );
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = None;
                (canvas::event::Status::Captured, None)
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 20.0,
                };
                (
                    canvas::event::Status::Captured,
                    Some(Message::ZoomAtCursor {
                        screen_x: pos.x,
                        screen_y: pos.y,
                        canvas_width: bounds.width,
                        canvas_height: bounds.height,
                        zoom_in: dy > 0.0,
                    }),
                )
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            // Background.
            frame.fill_rectangle(Point::ORIGIN, frame.size(), Color::from_rgb(0.07, 0.09, 0.13));
            let canvas_size = (frame.size().width, frame.size().height);
            let view = &self.app.state.view;
            let Some(sector) = self.app.state.sector.as_ref() else {
                return;
            };
            let active_runway = self
                .app
                .state
                .session
                .as_ref()
                .map(|session| session.state.active_runway.as_str())
                .unwrap_or(sector.active_runway.as_str());
            draw_sector(frame, view, canvas_size, sector, active_runway);
            if let Some(session) = self.app.state.session.as_ref() {
                if matches!(self.app.state.auth.role(), Some(Role::Trainer)) {
                    draw_trainer_paths(frame, view, canvas_size, &session.state.aircraft);
                }
                draw_aircraft(frame, view, canvas_size, &session.state.aircraft, &self.app.state.view.focused_aircraft);
            }
        });
        vec![geom]
    }
}

fn draw_sector(
    frame: &mut Frame,
    view: &crate::core::state::ViewState,
    canvas_size: (f32, f32),
    sector: &SectorMetadata,
    active_runway: &str,
) {
    // CTR boundary.
    match sector.ctr {
        CtrShape::Square { half_extent_nm } => {
            let (cx, cy) = view.world_to_screen((0.0, 0.0), canvas_size);
            let side = 2.0 * half_extent_nm * view.zoom_px_per_nm;
            let top_left = Point::new(cx - side / 2.0, cy - side / 2.0);
            let rect = Path::rectangle(top_left, iced::Size::new(side, side));
            frame.stroke(
                &rect,
                Stroke::default()
                    .with_color(Color::from_rgb(0.35, 0.45, 0.6))
                    .with_width(1.5),
            );
        }
    }

    // Runways: draw a line between the `hold_<end>` spawn points associated
    // with each runway's two ends (the held threshold of each direction).
    for rwy in &sector.runways {
        if rwy.ends.len() != 2 {
            continue;
        }
        let Some(a) = lookup_named(&sector.spawn_points, &format!("hold_{}", rwy.ends[0])) else { continue };
        let Some(b) = lookup_named(&sector.spawn_points, &format!("hold_{}", rwy.ends[1])) else { continue };
        let pa = view.world_to_screen((a.x_nm, a.y_nm), canvas_size);
        let pb = view.world_to_screen((b.x_nm, b.y_nm), canvas_size);
        frame.stroke(
            &Path::line(Point::new(pa.0, pa.1), Point::new(pb.0, pb.1)),
            Stroke::default()
                .with_color(Color::from_rgb(0.85, 0.85, 0.9))
                .with_width(6.0),
        );
        frame.fill_text(Text {
            content: rwy.ends[0].clone(),
            position: Point::new(pa.0 + 8.0, pa.1 + 8.0),
            color: Color::from_rgb(0.95, 0.95, 0.98),
            size: 13.0.into(),
            ..Text::default()
        });
        frame.fill_text(Text {
            content: rwy.ends[1].clone(),
            position: Point::new(pb.0 + 8.0, pb.1 - 10.0),
            color: Color::from_rgb(0.95, 0.95, 0.98),
            size: 13.0.into(),
            ..Text::default()
        });
    }

    draw_ground_layout(frame, view, canvas_size, sector);

    // ILS localiser: project from threshold opposite to the runway course
    // from 10 NM final to the runway threshold.
    for ils in &sector.ils {
        if !ils.visible_when_active || ils.runway != active_runway {
            continue;
        }
        let Some(origin) = lookup_named(&sector.spawn_points, &format!("hold_{}", ils.runway)) else { continue };
        let approach_rad = (ils.course_deg as f32 + 180.0).to_radians();
        let dx = approach_rad.sin();
        let dy = approach_rad.cos();
        let start_world = (origin.x_nm + dx * 10.0, origin.y_nm + dy * 10.0);
        let s = view.world_to_screen(start_world, canvas_size);
        let e = view.world_to_screen((origin.x_nm, origin.y_nm), canvas_size);
        frame.stroke(
            &Path::line(Point::new(s.0, s.1), Point::new(e.0, e.1)),
            Stroke::default()
                .with_color(Color::from_rgb(0.3, 0.7, 0.5))
                .with_width(1.2),
        );
    }

    // Outer fix labels.
    for np in &sector.outer_points {
        let (sx, sy) = view.world_to_screen((np.point.x_nm, np.point.y_nm), canvas_size);
        let dot = Path::circle(Point::new(sx, sy), 3.0);
        frame.fill(&dot, Color::from_rgb(0.6, 0.6, 0.4));
        frame.fill_text(Text {
            content: np.name.clone(),
            position: Point::new(sx + 6.0, sy - 6.0),
            color: Color::from_rgb(0.7, 0.7, 0.5),
            size: 11.0.into(),
            ..Text::default()
        });
    }
}

fn draw_ground_layout(
    frame: &mut Frame,
    view: &crate::core::state::ViewState,
    canvas_size: (f32, f32),
    sector: &SectorMetadata,
) {
    let taxiway_label_color = Color::from_rgb(0.92, 0.45, 0.74);
    let main_taxiway_x = lookup_named(&sector.spawn_points, "taxi_main_north")
        .map(|point| point.x_nm)
        .unwrap_or(-0.95);

    if let (Some(main_south), Some(main_north)) = (
        lookup_named(&sector.spawn_points, "taxi_main_south"),
        lookup_named(&sector.spawn_points, "taxi_main_north"),
    ) {
        let south = view.world_to_screen((main_south.x_nm, main_south.y_nm), canvas_size);
        let north = view.world_to_screen((main_north.x_nm, main_north.y_nm), canvas_size);
        frame.stroke(
            &Path::line(Point::new(south.0, south.1), Point::new(north.0, north.1)),
            Stroke::default()
                .with_color(Color::from_rgb(0.55, 0.63, 0.72))
                .with_width(2.0),
        );
        let label_pos = view.world_to_screen((main_south.x_nm - 0.015, 0.0), canvas_size);
        frame.fill_text(Text {
            content: "MAIN".into(),
            position: Point::new(label_pos.0 - 24.0, label_pos.1),
            color: taxiway_label_color,
            size: 11.0.into(),
            ..Text::default()
        });
    }

    for (exit_name, label) in [
        ("exit_a_to_main", "1"),
        ("exit_b_to_main", "2"),
        ("exit_c_to_main", "3"),
        ("exit_d_to_main", "4"),
    ] {
        let Some(exit) = lookup_named(&sector.spawn_points, exit_name) else { continue };
        let start = view.world_to_screen((0.0, exit.y_nm), canvas_size);
        let end = view.world_to_screen((exit.x_nm, exit.y_nm), canvas_size);
        frame.stroke(
            &Path::line(Point::new(start.0, start.1), Point::new(end.0, end.1)),
            Stroke::default()
                .with_color(Color::from_rgb(0.55, 0.63, 0.72))
                .with_width(1.5),
        );
        let label_pos = view.world_to_screen((exit.x_nm * 0.4, exit.y_nm + 0.015), canvas_size);
        frame.fill_text(Text {
            content: label.into(),
            position: Point::new(label_pos.0 - 4.0, label_pos.1),
            color: taxiway_label_color,
            size: 11.0.into(),
            ..Text::default()
        });
    }

    for stand_index in 1..=10 {
        let stand_name = format!("stand_{stand_index}");
        let Some(stand) = lookup_named(&sector.spawn_points, &stand_name) else { continue };
        let stand_screen = view.world_to_screen((stand.x_nm, stand.y_nm), canvas_size);
        let stop_line = view.world_to_screen((main_taxiway_x, stand.y_nm), canvas_size);
        frame.stroke(
            &Path::line(
                Point::new(stand_screen.0, stand_screen.1),
                Point::new(stop_line.0, stop_line.1),
            ),
            Stroke::default()
                .with_color(Color::from_rgb(0.55, 0.63, 0.72))
                .with_width(1.3),
        );
        frame.fill(
            &Path::circle(Point::new(stand_screen.0, stand_screen.1), 3.5),
            Color::from_rgb(0.45, 0.52, 0.60),
        );
        frame.fill_text(Text {
            content: stand_index.to_string(),
            position: Point::new(stand_screen.0 - 18.0, stand_screen.1 + 4.0),
            color: Color::from_rgb(0.84, 0.86, 0.90),
            size: 10.0.into(),
            ..Text::default()
        });
    }

    if let (Some(first), Some(last)) = (
        lookup_named(&sector.spawn_points, "stand_1"),
        lookup_named(&sector.spawn_points, "stand_10"),
    ) {
        let top_left = view.world_to_screen((first.x_nm - 0.25, last.y_nm + 0.18), canvas_size);
        let bottom_right =
            view.world_to_screen((first.x_nm + 0.25, first.y_nm - 0.18), canvas_size);
        frame.stroke(
            &Path::rectangle(
                Point::new(top_left.0, top_left.1),
                iced::Size::new(bottom_right.0 - top_left.0, bottom_right.1 - top_left.1),
            ),
            Stroke::default()
                .with_color(Color::from_rgb(0.38, 0.44, 0.50))
                .with_width(1.0),
        );
    }
}

fn lookup_named<'a>(points: &'a [NamedPoint], name: &str) -> Option<&'a WorldPoint> {
    points.iter().find(|p| p.name == name).map(|p| &p.point)
}

fn draw_aircraft(
    frame: &mut Frame,
    view: &crate::core::state::ViewState,
    canvas_size: (f32, f32),
    aircraft: &[AircraftState],
    focused: &Option<atc_shared::ids::AircraftId>,
) {
    for ac in aircraft {
        let (sx, sy) = view.world_to_screen((ac.x_nm, ac.y_nm), canvas_size);
        let is_focused = focused.as_ref().map(|f| f == &ac.aircraft_id).unwrap_or(false);
        let radius = if is_focused { 7.0 } else { 5.0 };
        let color = if is_focused {
            Color::from_rgb(1.0, 0.85, 0.4)
        } else {
            Color::from_rgb(0.8, 0.85, 1.0)
        };
        frame.fill(&Path::circle(Point::new(sx, sy), radius), color);
        draw_airborne_vector(frame, view, canvas_size, ac);
        let frame_color = match ac.squawk_mode {
            SquawkMode::Off => Color::from_rgb(0.5, 0.5, 0.5),
            SquawkMode::Standby => Color::from_rgb(1.0, 1.0, 1.0),
            SquawkMode::Charlie => Color::from_rgb(0.3, 0.9, 0.4),
        };
        let (label, label_width, label_height) = if ac.assumed_by_student || is_focused {
            let next_waypoint = ac.next_waypoint.clone().unwrap_or_else(|| "-".into());
            let assigned_altitude = ac.assigned_altitude_ft
                .map(|value| format!("A{:03}", altitude_to_a_format(value as f32)))
                .unwrap_or_else(|| "---".into());
            (
                format!(
                    "{}  {}|WPT {}|CUR A{:03}|ASG {}",
                    ac.callsign,
                    ac.aircraft_type,
                    next_waypoint,
                    altitude_to_a_format(ac.altitude_ft),
                    assigned_altitude,
                ),
                168.0,
                48.0,
            )
        } else {
            (ac.callsign.clone(), 92.0, 18.0)
        };
        frame.fill(
            &Path::rectangle(
                Point::new(sx + 7.0, sy - label_height + 2.0),
                iced::Size::new(label_width, label_height),
            ),
            Color::from_rgba(0.10, 0.12, 0.16, 0.88),
        );
        frame.stroke(
            &Path::rectangle(
                Point::new(sx + 7.0, sy - label_height + 2.0),
                iced::Size::new(label_width, label_height),
            ),
            Stroke::default().with_color(frame_color).with_width(1.0),
        );
        if ac.assumed_by_student || is_focused {
            let mut lines = label.split('|');
            let line1 = lines.next().unwrap_or_default();
            let line2 = lines.next().unwrap_or_default();
            let line3_left = lines.next().unwrap_or_default();
            let line3_right = lines.next().unwrap_or_default();

            frame.fill_text(Text {
                content: line1.into(),
                position: Point::new(sx + 11.0, sy - label_height + 15.0),
                color: Color::from_rgb(0.9, 0.92, 1.0),
                size: 11.0.into(),
                ..Text::default()
            });
            frame.fill_text(Text {
                content: line2.into(),
                position: Point::new(sx + 11.0, sy - label_height + 28.0),
                color: Color::from_rgb(0.9, 0.92, 1.0),
                size: 11.0.into(),
                ..Text::default()
            });
            frame.fill_text(Text {
                content: line3_left.into(),
                position: Point::new(sx + 11.0, sy - label_height + 41.0),
                color: Color::from_rgb(0.9, 0.92, 1.0),
                size: 11.0.into(),
                ..Text::default()
            });
            frame.fill_text(Text {
                content: line3_right.into(),
                position: Point::new(sx + 88.0, sy - label_height + 41.0),
                color: Color::from_rgb(0.9, 0.92, 1.0),
                size: 11.0.into(),
                ..Text::default()
            });
        } else {
            frame.fill_text(Text {
                content: label,
                position: Point::new(sx + 11.0, sy - 2.0),
                color: Color::from_rgb(0.9, 0.92, 1.0),
                size: 11.0.into(),
                ..Text::default()
            });
        }
    }
}

fn altitude_to_a_format(value_ft: f32) -> i32 {
    (value_ft / 100.0).round() as i32
}

fn draw_airborne_vector(
    frame: &mut Frame,
    view: &crate::core::state::ViewState,
    canvas_size: (f32, f32),
    aircraft: &AircraftState,
) {
    if aircraft.status != atc_shared::aircraft::AircraftStatus::Airborne {
        return;
    }
    let speed = aircraft.air_speed_kt.unwrap_or(aircraft.ground_speed_kt);
    let length_nm = speed / 60.0;
    let Some((dx, dy)) = vector_direction(aircraft) else {
        return;
    };
    let target = (aircraft.x_nm + dx * length_nm, aircraft.y_nm + dy * length_nm);
    let start = view.world_to_screen((aircraft.x_nm, aircraft.y_nm), canvas_size);
    let end = view.world_to_screen(target, canvas_size);
    frame.stroke(
        &Path::line(Point::new(start.0, start.1), Point::new(end.0, end.1)),
        Stroke::default()
            .with_color(Color::from_rgb(0.3, 0.9, 0.4))
            .with_width(1.0),
    );
}

fn vector_direction(aircraft: &AircraftState) -> Option<(f32, f32)> {
    if let Some(active_path) = &aircraft.active_path {
        for point in &active_path.points {
            let dx = point.x_nm - aircraft.x_nm;
            let dy = point.y_nm - aircraft.y_nm;
            let magnitude = (dx * dx + dy * dy).sqrt();
            if magnitude > 0.01 {
                return Some((dx / magnitude, dy / magnitude));
            }
        }
    }

    if let Some(next_waypoint) = aircraft.next_waypoint.as_deref() {
        match next_waypoint {
            "NORTH" => return Some((0.0, 1.0)),
            "EAST" => return Some((1.0, 0.0)),
            "SOUTH" => return Some((0.0, -1.0)),
            "WEST" => return Some((-1.0, 0.0)),
            _ => {}
        }
    }

    match aircraft.assigned_runway.as_deref() {
        Some("18") => Some((0.0, -1.0)),
        Some("36") => Some((0.0, 1.0)),
        _ => None,
    }
}

fn draw_trainer_paths(
    frame: &mut Frame,
    view: &crate::core::state::ViewState,
    canvas_size: (f32, f32),
    aircraft: &[AircraftState],
) {
    for aircraft in aircraft {
        let path = aircraft
            .draft_path
            .as_ref()
            .or(aircraft.active_path.as_ref());
        let Some(path) = path else { continue };
        let mut previous = (aircraft.x_nm, aircraft.y_nm);
        for point in &path.points {
            let start = view.world_to_screen(previous, canvas_size);
            let end = view.world_to_screen((point.x_nm, point.y_nm), canvas_size);
            frame.stroke(
                &Path::line(Point::new(start.0, start.1), Point::new(end.0, end.1)),
                Stroke::default()
                    .with_color(Color::from_rgb(0.9, 0.7, 0.25))
                    .with_width(1.0),
            );
            let label = match point.target_altitude {
                TargetAltitude::Gnd => "GND".to_string(),
                TargetAltitude::MslFt { value_ft } => value_ft.to_string(),
            };
            frame.fill_text(Text {
                content: format!("#{} {}kt {}", point.seq, point.target_speed_kt as i32, label),
                position: Point::new(end.0 + 4.0, end.1 - 4.0),
                color: Color::from_rgb(0.95, 0.8, 0.4),
                size: 10.0.into(),
                ..Text::default()
            });
            previous = (point.x_nm, point.y_nm);
        }
    }
}
