use crate::ids::SectorId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WorldPoint {
    pub x_nm: f32,
    pub y_nm: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedPoint {
    pub name: String,
    pub point: WorldPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum CtrShape {
    Square { half_extent_nm: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunwayInfo {
    pub runway_id: String,
    pub surface: String,
    pub length_ft: i32,
    pub ends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidInfo {
    pub sid_id: String,
    pub family: String,
    pub runway: String,
    pub initial_heading_deg: i32,
    pub paired_sid_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IlsInfo {
    pub runway: String,
    pub course_deg: i32,
    pub visible_when_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorMetadata {
    pub sector_id: SectorId,
    pub airport_name: String,
    pub airport_reference_point: WorldPoint,
    pub ctr: CtrShape,
    pub active_runway: String,
    pub runways: Vec<RunwayInfo>,
    pub sids: Vec<SidInfo>,
    pub ils: Vec<IlsInfo>,
    pub outer_points: Vec<NamedPoint>,
    pub spawn_points: Vec<NamedPoint>,
}

pub fn builtin_sector() -> SectorMetadata {
    SectorMetadata {
        sector_id: SectorId::new("airport_v1"),
        airport_name: "Training Airport".into(),
        airport_reference_point: WorldPoint { x_nm: 0.0, y_nm: 0.0 },
        ctr: CtrShape::Square { half_extent_nm: 10.0 },
        active_runway: "18".into(),
        runways: vec![RunwayInfo {
            runway_id: "18_36".into(),
            surface: "asphalt".into(),
            length_ft: 4000,
            ends: vec!["18".into(), "36".into()],
        }],
        sids: vec![
            SidInfo { sid_id: "north1a".into(), family: "north1".into(), runway: "18".into(), initial_heading_deg: 360, paired_sid_id: "north1b".into() },
            SidInfo { sid_id: "east1a".into(), family: "east1".into(), runway: "18".into(), initial_heading_deg: 90, paired_sid_id: "east1b".into() },
            SidInfo { sid_id: "south1a".into(), family: "south1".into(), runway: "18".into(), initial_heading_deg: 180, paired_sid_id: "south1b".into() },
            SidInfo { sid_id: "west1a".into(), family: "west1".into(), runway: "18".into(), initial_heading_deg: 270, paired_sid_id: "west1b".into() },
            SidInfo { sid_id: "north1b".into(), family: "north1".into(), runway: "36".into(), initial_heading_deg: 360, paired_sid_id: "north1a".into() },
            SidInfo { sid_id: "east1b".into(), family: "east1".into(), runway: "36".into(), initial_heading_deg: 90, paired_sid_id: "east1a".into() },
            SidInfo { sid_id: "south1b".into(), family: "south1".into(), runway: "36".into(), initial_heading_deg: 180, paired_sid_id: "south1a".into() },
            SidInfo { sid_id: "west1b".into(), family: "west1".into(), runway: "36".into(), initial_heading_deg: 270, paired_sid_id: "west1a".into() },
        ],
        ils: vec![
            IlsInfo { runway: "18".into(), course_deg: 176, visible_when_active: true },
            IlsInfo { runway: "36".into(), course_deg: 356, visible_when_active: true },
        ],
        outer_points: vec![
            NamedPoint { name: "NORTH".into(), point: WorldPoint { x_nm: 0.0, y_nm: 20.0 } },
            NamedPoint { name: "EAST".into(), point: WorldPoint { x_nm: 20.0, y_nm: 0.0 } },
            NamedPoint { name: "SOUTH".into(), point: WorldPoint { x_nm: 0.0, y_nm: -20.0 } },
            NamedPoint { name: "WEST".into(), point: WorldPoint { x_nm: -20.0, y_nm: 0.0 } },
        ],
        spawn_points: vec![
            NamedPoint { name: "stand_1".into(), point: WorldPoint { x_nm: -1.4, y_nm: -0.9 } },
            NamedPoint { name: "stand_2".into(), point: WorldPoint { x_nm: -1.1, y_nm: -0.9 } },
            NamedPoint { name: "stand_3".into(), point: WorldPoint { x_nm: -0.8, y_nm: -0.9 } },
            NamedPoint { name: "stand_4".into(), point: WorldPoint { x_nm: -0.5, y_nm: -0.9 } },
            NamedPoint { name: "stand_5".into(), point: WorldPoint { x_nm: -0.2, y_nm: -0.9 } },
            NamedPoint { name: "hold_18".into(), point: WorldPoint { x_nm: 0.0, y_nm: -0.3 } },
            NamedPoint { name: "hold_36".into(), point: WorldPoint { x_nm: 0.0, y_nm: 0.3 } },
            NamedPoint { name: "ctr_north".into(), point: WorldPoint { x_nm: 0.0, y_nm: 10.0 } },
            NamedPoint { name: "ctr_east".into(), point: WorldPoint { x_nm: 10.0, y_nm: 0.0 } },
            NamedPoint { name: "ctr_south".into(), point: WorldPoint { x_nm: 0.0, y_nm: -10.0 } },
            NamedPoint { name: "ctr_west".into(), point: WorldPoint { x_nm: -10.0, y_nm: 0.0 } },
            NamedPoint { name: "pattern_downwind_18".into(), point: WorldPoint { x_nm: 1.4, y_nm: -2.4 } },
            NamedPoint { name: "pattern_base_18".into(), point: WorldPoint { x_nm: 0.8, y_nm: -1.2 } },
            NamedPoint { name: "pattern_final_18".into(), point: WorldPoint { x_nm: 0.1, y_nm: -0.8 } },
            NamedPoint { name: "pattern_downwind_36".into(), point: WorldPoint { x_nm: -1.4, y_nm: 2.4 } },
            NamedPoint { name: "pattern_base_36".into(), point: WorldPoint { x_nm: -0.8, y_nm: 1.2 } },
            NamedPoint { name: "pattern_final_36".into(), point: WorldPoint { x_nm: -0.1, y_nm: 0.8 } },
            NamedPoint { name: "taxi_main_northbound".into(), point: WorldPoint { x_nm: -0.4, y_nm: 0.5 } },
            NamedPoint { name: "exit_c_to_main".into(), point: WorldPoint { x_nm: 0.6, y_nm: 0.2 } },
        ],
    }
}
