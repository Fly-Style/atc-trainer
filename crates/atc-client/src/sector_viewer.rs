use std::{cell::Cell, collections::BTreeMap, path::PathBuf, rc::Rc, sync::Arc};

use gpui::{
    actions, canvas, div, fill, point, prelude::*, px, rgb, size, App, Application, Bounds,
    Context, FocusHandle, KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    PathBuilder, PathPromptOptions, Pixels, Point, ScrollHandle, ScrollWheelEvent, Size, TextRun,
    TitlebarOptions, Window, WindowBounds, WindowOptions,
};

use crate::{
    aurora::{self, Airport, LabelKind, Sector},
    sector_camera::{MapBounds, SectorCamera},
};

const MAP: u32 = 0x0a0b0b;
const PANEL: u32 = 0x171a1b;
const BORDER: u32 = 0x333839;
const TEXT: u32 = 0xe0e4e1;
const MUTED: u32 = 0xa3adaa;
const SELECTED: u32 = 0x29443b;

actions!(
    sector_viewer,
    [
        OpenSector,
        FitAirport,
        NextAirport,
        PreviousAirport,
        CloseWindow
    ]
);

pub fn run(path: Option<PathBuf>) {
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("secondary-o", OpenSector, None),
            KeyBinding::new("secondary-0", FitAirport, None),
            KeyBinding::new("secondary-w", CloseWindow, None),
            KeyBinding::new("up", PreviousAirport, None),
            KeyBinding::new("down", NextAirport, None),
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1040.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("ATC Trainer · Sector Viewer".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(500.0), px(360.0))),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    focus.focus(window);
                    let mut viewer = SectorViewer {
                        focus,
                        sector: None,
                        selected: 0,
                        airport_scroll: ScrollHandle::new(),
                        airport_bounds: None,
                        camera: SectorCamera::default(),
                        canvas_bounds: Rc::default(),
                        drag_from: None,
                        busy: false,
                        error: None,
                    };
                    if let Some(path) = path {
                        viewer.load(path, cx);
                    }
                    viewer
                })
            },
        )
        .expect("Opening the sector viewer window");
        cx.activate(true);
    });
}

struct SectorViewer {
    focus: FocusHandle,
    sector: Option<(String, Arc<Sector>)>,
    selected: usize,
    airport_scroll: ScrollHandle,
    airport_bounds: Option<MapBounds>,
    camera: SectorCamera,
    canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    drag_from: Option<Point<Pixels>>,
    busy: bool,
    error: Option<String>,
}

impl SectorViewer {
    fn open(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        cx.notify();

        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Aurora sector (.isc)".into()),
        });
        cx.spawn(async move |this, cx| {
            let selection = picker.await;
            let _ = this.update(cx, |viewer, cx| {
                viewer.busy = false;
                match selection {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            viewer.load(path, cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => viewer.error = Some(error.to_string()),
                    Err(_) => viewer.error = Some("The file picker closed unexpectedly.".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();

        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let load = cx.background_executor().spawn(async move {
            anyhow::ensure!(
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("isc")),
                "Choose an Aurora .isc file (selected {}).",
                path.display()
            );
            Ok(aurora::load(&path)?)
        });

        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<Sector> = load.await;
            let _ = this.update(cx, |viewer, cx| {
                viewer.busy = false;
                match result {
                    Ok(sector) => {
                        viewer.sector = Some((file_name, Arc::new(sector)));
                        viewer.select(0, cx);
                    }
                    Err(error) => viewer.error = Some(format!("Could not open sector. {error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(airport) = self
            .sector
            .as_ref()
            .and_then(|(_, sector)| sector.airports.get(index))
        else {
            return;
        };
        self.selected = index;
        self.airport_scroll.scroll_to_item(index);
        self.airport_bounds = MapBounds::from_points(
            airport
                .lines
                .iter()
                .flat_map(|line| [line.start, line.end])
                .chain(
                    airport
                        .fills
                        .iter()
                        .flat_map(|fill| fill.vertices.iter().copied()),
                ),
        );
        self.fit(cx);
    }

    fn fit(&mut self, cx: &mut Context<Self>) {
        self.camera = SectorCamera::default();
        self.drag_from = None;
        cx.notify();
    }

    fn start_pan(&mut self, event: &MouseDownEvent, window: &mut Window, _: &mut Context<Self>) {
        self.focus.focus(window);
        if self.airport_bounds.is_some() {
            self.drag_from = Some(event.position);
        }
    }

    fn pan(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.drag_from = None;
        } else if let Some(previous) = self.drag_from.as_mut() {
            let delta = event.position - *previous;
            *previous = event.position;
            self.camera.pan([delta.x.into(), delta.y.into()]);
            cx.notify();
        }
    }

    fn zoom(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.airport_bounds.is_none() {
            return;
        }
        let bounds = self.canvas_bounds.get();
        let cursor = event.position - bounds.origin;
        let delta = f64::from(event.delta.pixel_delta(px(34.0)).y);
        self.camera.zoom(
            (delta * 0.005).exp(),
            [cursor.x.into(), cursor.y.into()],
            viewport(bounds),
        );
        cx.stop_propagation();
        cx.notify();
    }

    fn map_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if self.airport_bounds.is_none()
            || key.modifiers.control
            || key.modifiers.platform
            || key.modifiers.alt
        {
            return;
        }
        if key.modifiers.shift {
            let offset = match key.key.as_str() {
                "left" => Some([30.0, 0.0]),
                "right" => Some([-30.0, 0.0]),
                "up" => Some([0.0, 30.0]),
                "down" => Some([0.0, -30.0]),
                _ => None,
            };
            if let Some(offset) = offset {
                self.camera.pan(offset);
                cx.notify();
                return;
            }
        }
        let factor = match key.key_char.as_deref().unwrap_or(&key.key) {
            "+" | "=" => 1.25,
            "-" => 0.8,
            _ => return,
        };
        let size = viewport(self.canvas_bounds.get());
        self.camera.zoom(factor, size.map(|axis| axis / 2.0), size);
        cx.notify();
    }
}

impl Render for SectorViewer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sector = self.sector.as_ref().map(|(_, sector)| Arc::clone(sector));
        let selected = self.selected;
        let map_bounds = self.airport_bounds;
        let canvas_bounds = Rc::clone(&self.canvas_bounds);
        let camera = self.camera;
        let map = canvas(
            move |bounds, _, _| canvas_bounds.set(bounds),
            move |bounds, _, window, cx| {
                if let (Some(sector), Some(map_bounds)) = (&sector, map_bounds) {
                    paint_airport(
                        &sector.airports[selected],
                        camera,
                        map_bounds,
                        bounds,
                        window,
                        cx,
                    );
                }
            },
        )
        .size_full();

        let sidebar = self.sector.as_ref().map(|(_, sector)| {
            div()
                .flex()
                .flex_col()
                .w(px(132.0))
                .flex_shrink_0()
                .bg(rgb(PANEL))
                .border_r_1()
                .border_color(rgb(BORDER))
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child("AIRPORTS"),
                )
                .child(
                    div()
                        .id("airports")
                        .min_h_0()
                        .track_scroll(&self.airport_scroll)
                        .overflow_y_scroll()
                        .children(sector.airports.iter().enumerate().map(|(index, airport)| {
                            div()
                                .id(index)
                                .px_3()
                                .py_1()
                                .text_sm()
                                .cursor_pointer()
                                .when(index == selected, |row| row.bg(rgb(SELECTED)))
                                .hover(|row| row.bg(rgb(0x303a37)))
                                .child(airport.id.clone())
                                .on_click(
                                    cx.listener(move |viewer, _, _, cx| viewer.select(index, cx)),
                                )
                        })),
                )
        });

        let toolbar = div()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .h(px(43.0))
            .flex_shrink_0()
            .bg(rgb(PANEL))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                toolbar_button(
                    "open-sector",
                    if self.busy {
                        "Opening…"
                    } else {
                        "Open Sector…"
                    },
                )
                .when(!self.busy, |button| {
                    button.on_click(cx.listener(|viewer, _, _, cx| viewer.open(cx)))
                }),
            )
            .when_some(self.sector.as_ref(), |toolbar, (name, _)| {
                toolbar
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(MUTED))
                            .child(name.clone()),
                    )
                    .child(
                        toolbar_button("fit", "Fit Airport")
                            .on_click(cx.listener(|viewer, _, _, cx| viewer.fit(cx))),
                    )
            });

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::map_key))
            .on_action(cx.listener(|viewer, _: &OpenSector, _, cx| viewer.open(cx)))
            .on_action(cx.listener(|viewer, _: &FitAirport, _, cx| viewer.fit(cx)))
            .on_action(cx.listener(|viewer, _: &NextAirport, _, cx| {
                viewer.select(viewer.selected + 1, cx)
            }))
            .on_action(cx.listener(|viewer, _: &PreviousAirport, _, cx| {
                if viewer.selected > 0 {
                    viewer.select(viewer.selected - 1, cx);
                }
            }))
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(MAP))
            .text_sm()
            .text_color(rgb(TEXT))
            .child(toolbar)
            .when_some(self.error.as_ref(), |root, error| {
                root.child(
                    div()
                        .flex_shrink_0()
                        .max_h(px(84.0))
                        .id("load-error")
                        .overflow_y_scroll()
                        .px_3()
                        .py_2()
                        .bg(rgb(0x332121))
                        .text_color(rgb(0xffc9bd))
                        .child(error.clone()),
                )
            })
            .child(
                div().flex().flex_1().min_h_0().children(sidebar).child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::start_pan))
                        .on_mouse_up(MouseButton::Left, cx.listener(|viewer, _, _, _| viewer.drag_from = None))
                        .on_mouse_up_out(MouseButton::Left, cx.listener(|viewer, _, _, _| viewer.drag_from = None))
                        .on_mouse_move(cx.listener(Self::pan))
                        .on_scroll_wheel(cx.listener(Self::zoom))
                        .child(map)
                        .when_some(
                            self.sector.as_ref().and_then(|(_, sector)| sector.airports.get(selected)),
                            |map, airport| {
                                map.child(
                                    div().absolute().top_3().left_4().child(airport.id.clone()),
                                )
                                .child(
                                    div().absolute().top_3().right_4().text_color(rgb(MUTED)).child("↑ N"),
                                )
                            },
                        )
                        .when(self.sector.is_none(), |map| {
                            map.child(
                                div().absolute().inset_0().flex().items_center().justify_center()
                                    .text_color(rgb(MUTED))
                                    .child("Choose a local Aurora .isc file to view its airport layout."),
                            )
                        }),
                ),
            )
            .when(self.sector.is_some(), |root| {
                root.child(
                    div().px_3().py_1().flex_shrink_0().bg(rgb(PANEL)).border_t_1()
                        .border_color(rgb(BORDER)).text_xs().text_color(rgb(MUTED))
                        .child("Drag or Shift+arrows to pan · Scroll or +/− to zoom · ↑/↓ to change airport"),
                )
            })
    }
}

fn toolbar_button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_shrink_0()
        .px_2()
        .py_1()
        .bg(rgb(0x252a2b))
        .border_1()
        .border_color(rgb(BORDER))
        .hover(|button| button.bg(rgb(0x343b3b)))
        .cursor_pointer()
        .child(label)
}

fn viewport(bounds: Bounds<Pixels>) -> [f64; 2] {
    [bounds.size.width.into(), bounds.size.height.into()]
}

fn paint_airport(
    airport: &Airport,
    camera: SectorCamera,
    map_bounds: MapBounds,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let transform = camera.layout(map_bounds, viewport(bounds));
    let project = |position| {
        let [x, y] = transform.project(position);
        bounds.origin + point(px(x as f32), px(y as f32))
    };

    // Fills share the same transform as the linework and are painted beneath it.
    for fill in &airport.fills {
        if let Some((first, rest)) = fill.vertices.split_first() {
            let mut path = PathBuilder::fill();
            path.move_to(project(*first));
            for vertex in rest {
                path.line_to(project(*vertex));
            }
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(fill.color));
            }
        }
    }

    // Batch equal styles, with runway outlines drawn last.
    let mut paths = BTreeMap::new();
    for line in &airport.lines {
        let path = paths
            .entry((line.runway, line.color))
            .or_insert_with(|| PathBuilder::stroke(px(if line.runway { 1.35 } else { 0.9 })));
        path.move_to(project(line.start));
        path.line_to(project(line.end));
    }
    for ((_, color), path) in paths {
        if let Ok(path) = path.build() {
            window.paint_path(path, rgb(color));
        }
    }
    paint_labels(airport, project, bounds, window, cx);
}

fn paint_labels(
    airport: &Airport,
    project: impl Fn([f64; 2]) -> Point<Pixels>,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let font = window.text_style().font();
    let mut occupied = Vec::new();
    // Give the threshold designators first choice of space at every zoom level.
    for kind in [LabelKind::Runway, LabelKind::Stand] {
        let runway = kind == LabelKind::Runway;
        let font_size = px(if runway { 13.0 } else { 10.0 });
        let height = font_size + px(4.0);
        let color = rgb(if runway { 0xf0f1ed } else { 0xe0ce9c });
        for label in airport.labels.iter().filter(|label| label.kind == kind) {
            let anchor = project(label.position);
            if !bounds.contains(&anchor) {
                continue;
            }
            let run = TextRun {
                len: label.text.len(),
                font: font.clone(),
                color: color.into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let text =
                window
                    .text_system()
                    .shape_line(label.text.clone().into(), font_size, &[run], None);
            let gap = px(if runway { 8.0 } else { 4.0 });
            let dimensions = size(text.width + px(6.0), height);
            let Some(label_bounds) = label_placement(anchor, dimensions, gap, bounds, &occupied)
            else {
                continue;
            };
            occupied.push(label_bounds);
            if !runway
                && !occupied
                    .iter()
                    .any(|label_bounds| label_bounds.contains(&anchor))
            {
                window.paint_quad(fill(
                    Bounds::new(anchor - point(px(1.0), px(1.0)), size(px(2.0), px(2.0))),
                    color,
                ));
            }
            window.paint_quad(fill(label_bounds, gpui::rgba(0x0a0b0be8)));
            let _ = text.paint(
                label_bounds.origin + point(px(3.0), px(0.0)),
                height,
                window,
                cx,
            );
        }
    }
}

fn label_placement(
    anchor: Point<Pixels>,
    dimensions: Size<Pixels>,
    gap: Pixels,
    map: Bounds<Pixels>,
    occupied: &[Bounds<Pixels>],
) -> Option<Bounds<Pixels>> {
    if dimensions.height > map.size.height || dimensions.width > map.size.width {
        return None;
    }
    let vertical_margin = dimensions.height / 2.0;
    let y = anchor.y.clamp(
        map.origin.y + vertical_margin,
        map.bottom_right().y - vertical_margin,
    ) - vertical_margin;
    [anchor.x + gap, anchor.x - gap - dimensions.width]
        .into_iter()
        .map(|x| Bounds::new(point(x, y), dimensions))
        .find(|candidate| {
            map.contains(&candidate.origin)
                && map.contains(&candidate.bottom_right())
                && !occupied.iter().any(|other| candidate.intersects(other))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_fit_on_either_side_and_never_overlap() {
        let map = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
        let anchor = point(px(65.), px(25.));
        let dimensions = size(px(24.), px(12.));
        let pick = |occupied: &[Bounds<Pixels>]| {
            label_placement(anchor, dimensions, px(4.), map, occupied)
        };
        let right = pick(&[]).unwrap();
        assert_eq!(right.origin.x, px(69.));
        let left = pick(&[right]).unwrap();
        assert_eq!(left.origin.x, px(37.));
        assert!(pick(&[right, left]).is_none());
        let edge = point(px(96.), px(25.));
        assert_eq!(
            label_placement(edge, dimensions, px(4.), map, &[])
                .unwrap()
                .origin
                .x,
            px(68.)
        );
        let top = point(px(20.), px(1.));
        let top_label = label_placement(top, dimensions, px(4.), map, &[]).unwrap();
        assert_eq!(top_label.origin.y, px(0.));
        let bottom = point(px(20.), px(99.));
        let bottom_label = label_placement(bottom, dimensions, px(4.), map, &[]).unwrap();
        assert_eq!(bottom_label.bottom_right().y, px(100.));
        assert!(label_placement(top, size(px(101.), px(12.)), px(4.), map, &[]).is_none());
    }
}
