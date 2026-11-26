//! Local airport linework and static surface polygons from Aurora sector packages.

use std::{
    collections::HashMap,
    fs, io,
    path::{Component, Path, PathBuf},
};

use thiserror::Error;

#[derive(Debug)]
pub struct Sector {
    pub airports: Vec<Airport>,
}

#[derive(Debug)]
pub struct Airport {
    pub id: String,
    pub lines: Vec<Line>,
    pub fills: Vec<Fill>,
}

/// Endpoints and vertices are [east, north] in metres from a local airport origin.
#[derive(Debug)]
pub struct Line {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub color: u32,
    pub runway: bool,
}

#[derive(Debug)]
pub struct Fill {
    pub vertices: Vec<[f64; 2]>,
    pub color: u32,
}

#[derive(Debug, Error)]
pub enum AuroraError {
    #[error("Cannot read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("Invalid {record} in {path}, line {line}")]
    InvalidRecord {
        path: PathBuf,
        line: usize,
        record: &'static str,
    },
    #[error("Invalid include path {value:?} in {path}, line {line}")]
    InvalidIncludePath {
        path: PathBuf,
        line: usize,
        value: String,
    },
    #[error("Include path {path} leads outside the sector package")]
    OutsidePackage { path: PathBuf },
    #[error("Missing sector include {reference} in {manifest}")]
    MissingInclude {
        reference: PathBuf,
        manifest: PathBuf,
    },
    #[error("No airport geometry found in {0}")]
    NoAirports(PathBuf),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Geometry {
    Linework,
    Fills,
}

pub fn load(path: &Path) -> Result<Sector, AuroraError> {
    let manifest = read(path)?;
    let package = canonicalize(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let include = package.join("Include");
    let mut roots = Vec::new();
    let mut references = Vec::new();
    let mut definitions = Vec::new();
    let mut section = "";
    let mut info_rows = 0;

    for (line, record) in records(&manifest) {
        if record.starts_with('[') {
            section = record;
            continue;
        }
        if section.eq_ignore_ascii_case("[INFO]") {
            info_rows += 1;
            if info_rows == 6 {
                for folder in record
                    .split(';')
                    .map(str::trim)
                    .filter(|folder| !folder.is_empty())
                {
                    roots.push(include.join(relative_path(folder, path, line)?));
                }
            }
        }
        if section.eq_ignore_ascii_case("[DEFINE]") {
            definitions.push((line, record));
        }
        let kind = if section.eq_ignore_ascii_case("[GEO]") {
            Geometry::Linework
        } else if section.eq_ignore_ascii_case("[FILLCOLOR]") {
            Geometry::Fills
        } else {
            continue;
        };
        let Some((tag, reference)) = record.split_once(';') else {
            continue;
        };
        if !tag.trim().eq_ignore_ascii_case("F") {
            continue;
        }
        let relative = relative_path(reference.trim_end_matches(';').trim(), path, line)?;
        let is_airspace = relative
            .iter()
            .next()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.eq_ignore_ascii_case("Airspace") || name.eq_ignore_ascii_case("Airspaces")
            });
        let is_dynamic_fill = kind == Geometry::Fills
            && relative
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("fcl"));
        if !is_airspace && !is_dynamic_fill {
            references.push((kind, relative));
        }
    }
    roots.push(include);
    let colors = load_colors(&package, &roots, path, definitions)?;

    let mut airports: Vec<Airport> = Vec::new();
    for (_, reference) in references
        .iter()
        .filter(|(kind, _)| *kind == Geometry::Linework)
    {
        let source = resolve_include(&package, &roots, reference, false, path)?;
        let lines = parse_lines(&source, &read(&source)?)?;
        if lines.is_empty() {
            continue;
        }
        let id = airport_id(reference);
        if let Some(airport) = airports.iter_mut().find(|airport| airport.id == id) {
            airport.lines.extend(lines);
        } else {
            airports.push(Airport {
                id,
                lines,
                fills: Vec::new(),
            });
        }
    }

    if airports.is_empty() {
        return Err(AuroraError::NoAirports(path.to_owned()));
    }
    // Regional TFL files can share the FILLCOLOR section with airport surfaces.
    for (_, reference) in references
        .iter()
        .filter(|(kind, _)| *kind == Geometry::Fills)
    {
        let id = airport_id(reference);
        if let Some(airport) = airports.iter_mut().find(|airport| airport.id == id) {
            let source = resolve_include(&package, &roots, reference, true, path)?;
            airport
                .fills
                .extend(parse_fills(&source, &read(&source)?, &colors)?);
        }
    }
    for airport in &mut airports {
        project(airport);
    }
    Ok(Sector { airports })
}

fn airport_id(reference: &Path) -> String {
    reference
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_uppercase()
}

fn load_colors(
    package: &Path,
    roots: &[PathBuf],
    manifest: &Path,
    definitions: Vec<(usize, &str)>,
) -> Result<HashMap<String, u32>, AuroraError> {
    let mut colors = HashMap::new();
    for (line, record) in definitions {
        let (label, value) = color_definition(manifest, line, record)?;
        if label.eq_ignore_ascii_case("F") {
            let reference = relative_path(value, manifest, line)?;
            let source = resolve_include(package, roots, &reference, false, manifest)?;
            for (line, record) in records(&read(&source)?) {
                let (label, value) = color_definition(&source, line, record)?;
                let color = rgb(value).ok_or_else(|| malformed(&source, line, "DEFINE color"))?;
                colors.insert(label.to_ascii_uppercase(), color);
            }
        } else {
            let color = rgb(value).ok_or_else(|| malformed(manifest, line, "DEFINE color"))?;
            colors.insert(label.to_ascii_uppercase(), color);
        }
    }
    Ok(colors)
}

fn color_definition<'a>(
    path: &Path,
    line: usize,
    record: &'a str,
) -> Result<(&'a str, &'a str), AuroraError> {
    record
        .trim_end_matches(';')
        .split_once(';')
        .map(|(label, value)| (label.trim(), value.trim()))
        .ok_or_else(|| malformed(path, line, "DEFINE record"))
}

fn read(path: &Path) -> Result<String, AuroraError> {
    fs::read_to_string(path).map_err(|source| AuroraError::Read {
        path: path.to_owned(),
        source,
    })
}

fn canonicalize(path: &Path) -> Result<PathBuf, AuroraError> {
    path.canonicalize().map_err(|source| AuroraError::Read {
        path: path.to_owned(),
        source,
    })
}

fn records(contents: &str) -> impl Iterator<Item = (usize, &str)> {
    contents
        .trim_start_matches('\u{feff}')
        .lines()
        .enumerate()
        .filter_map(|(row, line)| {
            let record = line.split("//").next().unwrap_or_default().trim();
            (!record.is_empty() && !record.starts_with(';')).then_some((row + 1, record))
        })
}

fn malformed(path: &Path, line: usize, record: &'static str) -> AuroraError {
    AuroraError::InvalidRecord {
        path: path.to_owned(),
        line,
        record,
    }
}

fn relative_path(value: &str, manifest: &Path, line: usize) -> Result<PathBuf, AuroraError> {
    // A leading slash in an Aurora reference denotes the include root.
    let normalized = value.replace('\\', "/");
    let relative = Path::new(normalized.trim_start_matches('/'));
    if relative.as_os_str().is_empty()
        || normalized.contains([':', '\0'])
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AuroraError::InvalidIncludePath {
            path: manifest.to_owned(),
            line,
            value: value.to_owned(),
        });
    }
    Ok(relative.to_owned())
}

fn resolve_include(
    package: &Path,
    roots: &[PathBuf],
    reference: &Path,
    root_fill_fallback: bool,
    manifest: &Path,
) -> Result<PathBuf, AuroraError> {
    let relative = std::iter::once(reference);
    // Aurora can discover ICAO-named TFL files at the include root.
    let root_fill = reference
        .file_name()
        .filter(|_| root_fill_fallback)
        .map(Path::new);
    for relative in relative.chain(root_fill) {
        for root in roots {
            let candidate = root.join(relative);
            match canonicalize(&candidate) {
                Ok(path) if path.starts_with(package) => return Ok(path),
                Ok(_) => return Err(AuroraError::OutsidePackage { path: candidate }),
                Err(AuroraError::Read { source, .. })
                    if source.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Err(AuroraError::MissingInclude {
        reference: reference.to_owned(),
        manifest: manifest.to_owned(),
    })
}

fn parse_lines(path: &Path, contents: &str) -> Result<Vec<Line>, AuroraError> {
    let mut lines = Vec::new();
    for (number, record) in records(contents) {
        let fields: Vec<_> = record.split(';').map(str::trim).collect();
        let [lat1, lon1, lat2, lon2, class, ..] = fields.as_slice() else {
            return Err(malformed(path, number, "GEO line"));
        };
        let (color, runway) = match class.to_ascii_uppercase().as_str() {
            "RUNWAY" => (0xe8e9e5, true),
            "TAXIWAY" => (0x878581, false),
            "TAXI_CENTER" => (0x65634b, false),
            "APRON" | "BUILDING" | "PIER" | "PARKING" => (0x747674, false),
            "STOPBAR" | "STOPLINE" => (0xd0be59, false),
            _ => continue,
        };
        let error = || malformed(path, number, "GEO coordinate");
        lines.push(Line {
            start: coordinates(lat1, lon1).ok_or_else(error)?,
            end: coordinates(lat2, lon2).ok_or_else(error)?,
            color,
            runway,
        });
    }
    Ok(lines)
}

fn parse_fills(
    path: &Path,
    contents: &str,
    colors: &HashMap<String, u32>,
) -> Result<Vec<Fill>, AuroraError> {
    let mut fills = Vec::new();
    let mut current: Option<(usize, Fill)> = None;
    let finish = |current: Option<(usize, Fill)>, fills: &mut Vec<Fill>| {
        if let Some((line, fill)) = current {
            if fill.vertices.len() < 3 {
                return Err(malformed(
                    path,
                    line,
                    "TFL polygon with fewer than three vertices",
                ));
            }
            fills.push(fill);
        }
        Ok(())
    };

    for (number, record) in records(contents) {
        let fields: Vec<_> = record
            .trim_end_matches(';')
            .split(';')
            .map(str::trim)
            .collect();
        match fields.as_slice() {
            [lat, lon] => {
                if let Some((_, fill)) = &mut current {
                    fill.vertices.push(
                        coordinates(lat, lon)
                            .ok_or_else(|| malformed(path, number, "TFL coordinate"))?,
                    );
                } else {
                    return Err(malformed(path, number, "TFL vertex without a polygon"));
                }
            }
            [kind, color, _width, _stroke, ..] if kind.eq_ignore_ascii_case("STATIC") => {
                finish(current.take(), &mut fills)?;
                current = Some((
                    number,
                    Fill {
                        vertices: Vec::new(),
                        color: rgb(color)
                            .or_else(|| colors.get(&color.to_ascii_uppercase()).copied())
                            .ok_or_else(|| malformed(path, number, "TFL fill color"))?,
                    },
                ));
            }
            _ => return Err(malformed(path, number, "TFL record")),
        }
    }
    finish(current, &mut fills)?;
    Ok(fills)
}

fn rgb(value: &str) -> Option<u32> {
    if let Some(digits) = value.strip_prefix('#') {
        if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        return Some(u32::from_str_radix(digits, 16).ok()? & 0xFFFFFF);
    }
    let mut channels = match value.strip_prefix('%') {
        Some(value) => value.split(':'),
        None => value.split(','),
    };
    let red = channels.next()?.trim().parse::<u8>().ok()?;
    let green = channels.next()?.trim().parse::<u8>().ok()?;
    let blue = channels.next()?.trim().parse::<u8>().ok()?;
    channels
        .next()
        .is_none()
        .then(|| u32::from_be_bytes([0, red, green, blue]))
}

fn coordinates(lat: &str, lon: &str) -> Option<[f64; 2]> {
    Some([
        angle(lon, b'E', b'W', 180.0)?,
        angle(lat, b'N', b'S', 90.0)?,
    ])
}

fn angle(value: &str, positive: u8, negative: u8, limit: f64) -> Option<f64> {
    let first = value.as_bytes().first()?.to_ascii_uppercase();
    let degrees = if first == positive || first == negative {
        let digits = &value[1..];
        let (degrees, minutes, seconds) =
            if digits.len() == 10 && digits.bytes().all(|byte| byte.is_ascii_digit()) {
                (
                    digits[..3].parse::<u32>().ok()?,
                    digits[3..5].parse::<u32>().ok()?,
                    digits[5..].parse::<u32>().ok()? as f64 / 1000.0,
                )
            } else {
                let mut parts = digits.splitn(3, '.');
                (
                    parts.next()?.parse::<u32>().ok()?,
                    parts.next()?.parse::<u32>().ok()?,
                    parts.next()?.parse::<f64>().ok()?,
                )
            };
        if minutes >= 60 || !(0.0..60.0).contains(&seconds) {
            return None;
        }
        let angle = f64::from(degrees) + f64::from(minutes) / 60.0 + seconds / 3600.0;
        if first == negative {
            -angle
        } else {
            angle
        }
    } else {
        value.parse::<f64>().ok()?
    };
    (-limit..=limit).contains(&degrees).then_some(degrees)
}

fn project(airport: &mut Airport) {
    let [lon, lat] = airport.lines[0].start;
    // shortcut: local metric projection at airport scale, use a regional projection for wider charts.
    let metres_per_degree = 6_371_000.0 * std::f64::consts::PI / 180.0;
    let metres_per_longitude = metres_per_degree * lat.to_radians().cos();
    let project = |[x, y]: [f64; 2]| {
        let longitude_delta = (x - lon + 180.0).rem_euclid(360.0) - 180.0;
        [
            longitude_delta * metres_per_longitude,
            (y - lat) * metres_per_degree,
        ]
    };
    for line in &mut airport.lines {
        line.start = project(line.start);
        line.end = project(line.end);
    }
    for fill in &mut airport.fills {
        for point in &mut fill.vertices {
            *point = project(*point);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let include = directory.path().join("Include/Fixture/Airports");
        fs::create_dir_all(&include).unwrap();
        let manifest = directory.path().join("coast.isc");
        fs::write(
            &manifest,
            "\u{feff}[INFO]\r\nS033.00.00.000\r\nW071.00.00.000\r\n60\r\n40\r\n0.0\r\nFixture\r\n7.0\r\n\r\n[DEFINE]\r\nF;colors.def;\r\n[GEO]\r\nF;\\Airports\\zzxy.geo\r\n[FILLCOLOR]\r\nF;/Region_TMA.tfl\r\nF;/Airports/zzxy.tfl\r\n",
        )
        .unwrap();
        fs::write(
            include.parent().unwrap().join("colors.def"),
            "GRASS;%39:61:48;\r\n",
        )
        .unwrap();
        fs::write(
            include.join("zzxy.geo"),
            "// Runway and taxiway\r\nS033.00.00.000;W071.00.00.000;-32.999;-71.000;RUNWAY;\r\n-33.000;-71.000;S0330000000;W0705959000;TAXIWAY;\r\n",
        )
        .unwrap();
        fs::write(
            include.join("zzxy.tfl"),
            "STATIC;grass;1;#00273d30;\r\nS033.00.00.000;W071.00.00.000;\r\n-32.999;-71.000;\r\n-32.999;-70.999;\r\n-32.9995;-70.9995;\r\n-33.000;-70.999;\r\n",
        )
        .unwrap();
        (directory, manifest)
    }

    #[test]
    fn loads_local_airport_with_aligned_linework_and_concave_fill() {
        let (_directory, manifest) = fixture();
        let sector = load(&manifest).unwrap();
        let [airport] = sector.airports.as_slice() else {
            panic!("expected one airport: {sector:?}")
        };
        let [runway, taxiway] = airport.lines.as_slice() else {
            panic!("expected two lines: {:?}", airport.lines)
        };
        let [fill] = airport.fills.as_slice() else {
            panic!("expected one fill: {:?}", airport.fills)
        };

        assert_eq!(airport.id, "ZZXY");
        assert!(runway.runway);
        assert!(!taxiway.runway);
        assert_eq!(runway.start, [0.0, 0.0]);
        assert!(runway.end[1] > 100.0);
        assert!(taxiway.end[0] > 20.0);
        assert_eq!(fill.vertices.len(), 5);
        assert_eq!(fill.vertices[0], runway.start);
        assert_eq!(fill.vertices[1], runway.end);
        assert_eq!(fill.color, 0x273d30);
    }

    #[test]
    fn names_missing_include_and_rejects_paths_outside_package() {
        let (directory, manifest) = fixture();
        fs::remove_file(directory.path().join("Include/Fixture/Airports/zzxy.geo")).unwrap();

        let error = load(&manifest).unwrap_err().to_string();
        assert!(error.contains("Airports/zzxy.geo"), "{error}");

        fs::write(&manifest, "[GEO]\nF;/../zzxy.geo\n").unwrap();
        let error = load(&manifest).unwrap_err().to_string();
        assert!(
            error.contains("include path") && error.contains(".."),
            "{error}"
        );
    }

    #[test]
    fn loads_reference_airport_when_package_is_available() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ukbu-aurora/UKBU.isc");
        if !manifest.is_file() {
            return;
        }
        let sector = load(&manifest).unwrap();
        let airport = sector
            .airports
            .iter()
            .find(|airport| airport.id == "UKBB")
            .unwrap();
        let runway = |x: std::ops::Range<f64>| {
            airport
                .lines
                .iter()
                .filter(move |line| line.runway && x.contains(&line.start[0]))
        };
        // The western runway is more than a kilometre west of the eastern one.
        assert!(runway(-2_000.0..-1_000.0).count() >= 20);
        assert!(runway(-30.0..300.0).count() >= 20);
        assert!(airport.fills.iter().any(|fill| fill.color == 0x273d30));
    }
}
