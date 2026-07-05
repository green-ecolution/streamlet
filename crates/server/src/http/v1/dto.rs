use serde::{Deserialize, Serialize};
use streamlet_core::domain::{CustomerId, Problem, Solution, Stop, VehicleId};
use streamlet_core::router::RouteGeometry;

use crate::service::{GeometryFormat, SolveResult};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SolveRequest {
    /// Domain problem; validation happens during deserialization.
    #[schema(value_type = Object)]
    pub problem: Problem,
    #[serde(default)]
    pub options: SolveRequestOptions,
}

#[derive(Debug, Default, Deserialize, utoipa::ToSchema)]
pub struct SolveRequestOptions {
    #[serde(default)]
    pub geometry: GeometryFormatDto,
    /// Solver time budget in milliseconds; clamped to the server-side maximum.
    pub time_limit_ms: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GeometryFormatDto {
    #[default]
    None,
    /// Encoded polylines (precision 6), one per leg, joined with `;`.
    Polyline,
    /// Currently identical to `polyline`; proper GeoJSON conversion is future work.
    Geojson,
}

impl From<GeometryFormatDto> for GeometryFormat {
    fn from(value: GeometryFormatDto) -> Self {
        match value {
            GeometryFormatDto::None => GeometryFormat::None,
            GeometryFormatDto::Polyline => GeometryFormat::Polyline,
            GeometryFormatDto::Geojson => GeometryFormat::GeoJson,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SolveResponse {
    pub routes: Vec<RouteDto>,
    #[schema(value_type = Vec<u32>)]
    pub unserved: Vec<CustomerId>,
    /// Meters.
    pub total_distance: f64,
    /// Seconds.
    pub total_travel_time: f64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RouteDto {
    #[schema(value_type = u32)]
    pub vehicle: VehicleId,
    #[schema(value_type = Vec<Object>)]
    pub stops: Vec<Stop>,
    /// Meters.
    pub distance: f64,
    /// Seconds.
    pub travel_time: f64,
    /// Seconds.
    pub wait_time: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub geometry: Option<RouteGeometry>,
}

impl From<SolveResult> for SolveResponse {
    fn from(result: SolveResult) -> Self {
        let SolveResult {
            solution,
            routes: geometries,
        } = result;
        let Solution {
            routes,
            unserved,
            total_distance,
            total_travel_time,
        } = solution;
        let routes = routes
            .into_iter()
            .zip(geometries)
            .map(|(r, g)| RouteDto {
                vehicle: r.vehicle,
                stops: r.stops,
                distance: r.distance.get(),
                travel_time: r.travel_time.get(),
                wait_time: r.wait_time.get(),
                geometry: g.geometry,
            })
            .collect();
        Self {
            routes,
            unserved,
            total_distance: total_distance.get(),
            total_travel_time: total_travel_time.get(),
        }
    }
}
