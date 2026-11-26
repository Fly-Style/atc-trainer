const FIT_FRACTION: f64 = 0.88;
const MIN_ZOOM: f64 = 0.25;
const MAX_ZOOM: f64 = 32.0;

#[derive(Clone, Copy)]
pub(crate) struct MapBounds {
    min: [f64; 2],
    max: [f64; 2],
}

impl MapBounds {
    pub(crate) fn from_points(points: impl IntoIterator<Item = [f64; 2]>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let mut bounds = Self {
            min: first,
            max: first,
        };
        for [x, y] in points {
            bounds.min = [bounds.min[0].min(x), bounds.min[1].min(y)];
            bounds.max = [bounds.max[0].max(x), bounds.max[1].max(y)];
        }
        Some(bounds)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SectorCamera {
    offset: [f64; 2],
    zoom: f64,
}

impl Default for SectorCamera {
    fn default() -> Self {
        Self {
            offset: [0.0; 2],
            zoom: 1.0,
        }
    }
}

impl SectorCamera {
    pub(crate) fn layout(&self, bounds: MapBounds, viewport: [f64; 2]) -> SectorTransform {
        let [min_x, min_y] = bounds.min;
        let [max_x, max_y] = bounds.max;
        let width = (max_x - min_x).max(1.0);
        let height = (max_y - min_y).max(1.0);
        let fit_x = viewport[0].max(1.0) / width;
        let fit_y = viewport[1].max(1.0) / height;
        SectorTransform {
            world_center: [min_x.midpoint(max_x), min_y.midpoint(max_y)],
            screen_center: [
                viewport[0] / 2.0 + self.offset[0],
                viewport[1] / 2.0 + self.offset[1],
            ],
            pixels_per_meter: fit_x.min(fit_y) * FIT_FRACTION * self.zoom,
        }
    }

    pub(crate) fn pan(&mut self, [dx, dy]: [f64; 2]) {
        self.offset[0] += dx;
        self.offset[1] += dy;
    }

    pub(crate) fn zoom(&mut self, factor: f64, cursor: [f64; 2], viewport: [f64; 2]) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let applied_factor = new_zoom / self.zoom;
        self.zoom = new_zoom;

        // Use the applied ratio so the point under the cursor stays fixed at the zoom limits.
        for axis in 0..2 {
            let anchor = cursor[axis] - viewport[axis] / 2.0;
            self.offset[axis] = anchor + (self.offset[axis] - anchor) * applied_factor;
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SectorTransform {
    world_center: [f64; 2],
    screen_center: [f64; 2],
    pixels_per_meter: f64,
}

impl SectorTransform {
    pub(crate) fn project(&self, [east, north]: [f64; 2]) -> [f64; 2] {
        [
            self.screen_center[0] + (east - self.world_center[0]) * self.pixels_per_meter,
            self.screen_center[1] - (north - self.world_center[1]) * self.pixels_per_meter,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_geometry_north_up_when_the_viewport_resizes() {
        let south_west = [100.0, 100.0];
        let north_east = [300.0, 200.0];
        let bounds = MapBounds::from_points([south_west, north_east]).unwrap();
        let camera = SectorCamera::default();
        for viewport in [[800.0, 400.0], [180.0, 500.0]] {
            let transform = camera.layout(bounds, viewport);
            let [west, south] = transform.project(south_west);
            let [east, north] = transform.project(north_east);
            assert!(0.0 < west && west < east && east < viewport[0]);
            assert!(0.0 < north && north < south && south < viewport[1]);
            assert_eq!(transform.project([200.0, 150.0]), viewport.map(|v| v / 2.0));
            assert!(((east - west) / (south - north) - 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn zoom_keeps_the_cursor_over_the_same_point_after_panning() {
        let bounds = MapBounds::from_points([[-100.0, -50.0], [100.0, 50.0]]).unwrap();
        let viewport = [600.0, 400.0];
        let point = [60.0, 25.0];
        let mut camera = SectorCamera::default();
        let before = camera.layout(bounds, viewport);
        let [x, y] = before.project(point);
        camera.pan([42.0, -20.0]);
        let cursor = [x + 42.0, y - 20.0];
        assert_eq!(camera.layout(bounds, viewport).project(point), cursor);

        // Even a wheel step that reaches the zoom limit must preserve the anchor.
        for (factor, expected_scale) in [(1.5, 1.5), (10_000.0, 32.0), (0.0001, 0.25)] {
            camera.zoom(factor, cursor, viewport);
            let after = camera.layout(bounds, viewport);
            let [x, y] = after.project(point);
            assert!((x - cursor[0]).abs() < 1e-9);
            assert!((y - cursor[1]).abs() < 1e-9);
            let relative_scale = after.pixels_per_meter / before.pixels_per_meter;
            assert!((relative_scale - expected_scale).abs() < 1e-9);
        }
    }
}
