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
                .with_width(3.0),
        );
    }

    // ILS localiser: project from threshold opposite to the runway course
    // (i.e., out toward where approach traffic is coming from).
    for ils in &sector.ils {
        if !ils.visible_when_active || ils.runway != active_runway {
            continue;
        }
        let Some(origin) = lookup_named(&sector.spawn_points, &format!("hold_{}", ils.runway)) else { continue };
        let approach_rad = (ils.course_deg as f32 + 180.0).to_radians();
        let dx = approach_rad.sin();
        let dy = approach_rad.cos();
        let length_nm = 8.0;
        let end = (origin.x_nm + dx * length_nm, origin.y_nm + dy * length_nm);
        let s = view.world_to_screen((origin.x_nm, origin.y_nm), canvas_size);
        let e = view.world_to_screen(end, canvas_size);
        frame.stroke(
            &Path::line(Point::new(s.0, s.1), Point::new(e.0, e.1)),
            Stroke::default()
                .with_color(Color::from_rgb(0.3, 0.7, 0.5))
                .with_width(1.0),
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
        frame.stroke(
            &Path::rectangle(Point::new(sx + 7.0, sy - 28.0), iced::Size::new(100.0, 26.0)),
            Stroke::default().with_color(frame_color).with_width(1.0),
        );
        let label = if ac.assumed_by_student || is_focused {
            format!(
                "{} {}\n{} ALT {:.0}/{:.0}",
                ac.callsign,
                ac.aircraft_type,
                ac.next_waypoint.clone().unwrap_or_else(|| "-".into()),
                ac.altitude_ft,
                ac.assigned_altitude_ft.unwrap_or_default()
            )
        } else {
            ac.callsign.clone()
        };
        frame.fill_text(Text {
            content: label,
            position: Point::new(sx + 9.0, sy - 14.0),
            color: Color::from_rgb(0.9, 0.92, 1.0),
            size: 11.0.into(),
            ..Text::default()
        });
    }
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
