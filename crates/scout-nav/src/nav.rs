use serde::{Deserialize, Serialize};
use crate::gnss::GnssFix;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Home {
    pub lat: f64,
    pub lon: f64,
    pub alt_m: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCfg {
    pub corridor_width_m: f64,
    pub waypoints: Vec<Point>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneCfg {
    pub zone_polygon: Vec<Point>,
}

#[derive(Debug, Clone)]
pub struct RthPolicy {
    pub grace_link_loss_s: u64,
    pub gnss_bad_fix_s: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionState {
    Idle,
    TransitToZone,
    OperateInZone,
    Rth,
    Land,
    Abort,
}

#[derive(Debug, Clone)]
pub struct NavOutput {
    pub state: MissionState,
    pub message: String,
}

pub struct NavEngine {
    home: Home,
    route: RouteCfg,
    zone: ZoneCfg,
    max_radius_m: f64,
    policy: RthPolicy,

    state: MissionState,
    gnss_bad_since: Option<time::OffsetDateTime>,
}

impl NavEngine {
    pub fn new(home: Home, route: RouteCfg, zone: ZoneCfg, max_radius_m: f64, policy: RthPolicy) -> Self {
        Self {
            home, route, zone, max_radius_m, policy,
            state: MissionState::TransitToZone,
            gnss_bad_since: None,
        }
    }

    pub fn step(&mut self, fix: GnssFix) -> NavOutput {
        let now = fix.ts;
        let q = &fix.quality;

        let gnss_ok = q.sats >= 6 && q.hdop <= 5.0 && q.fix_age_s <= 5;
        if !gnss_ok {
            self.gnss_bad_since.get_or_insert(now);
        } else {
            self.gnss_bad_since = None;
        }

        // Absolute max radius cap
        let d_home = haversine_m(self.home.lat, self.home.lon, fix.lat, fix.lon);
        if d_home > self.max_radius_m {
            self.state = MissionState::Abort;
            return NavOutput { state: self.state, message: format!("ABORT: exceeded max_radius_m ({}m)", d_home as i64) };
        }

        // GNSS degrade ladder
        if let Some(t0) = self.gnss_bad_since {
            let bad_s = (now - t0).whole_seconds().max(0) as u64;
            if bad_s >= self.policy.gnss_bad_fix_s {
                self.state = MissionState::Rth;
                return NavOutput { state: self.state, message: format!("RTH: GNSS bad for {}s (sats={}, hdop={}, age={}s)", bad_s, q.sats, q.hdop, q.fix_age_s) };
            }
        }

        // Geofence checks
        let in_corridor = point_in_corridor(&self.route, fix.lat, fix.lon);
        let in_zone = point_in_polygon(&self.zone.zone_polygon, fix.lat, fix.lon);

        self.state = match self.state {
            MissionState::TransitToZone => {
                if !in_corridor { MissionState::Rth }
                else if in_zone { MissionState::OperateInZone }
                else { MissionState::TransitToZone }
            }
            MissionState::OperateInZone => {
                if !in_zone { MissionState::Rth } else { MissionState::OperateInZone }
            }
            MissionState::Rth => MissionState::Rth,
            s => s,
        };

        let msg = match self.state {
            MissionState::TransitToZone => format!("TRANSIT: corridor_ok={}, zone={}", in_corridor, in_zone),
            MissionState::OperateInZone => "OPERATE: inside operation zone".to_string(),
            MissionState::Rth => format!("RTH: boundary violated (corridor_ok={}, zone={})", in_corridor, in_zone),
            MissionState::Abort => "ABORT".to_string(),
            MissionState::Land => "LAND".to_string(),
            MissionState::Idle => "IDLE".to_string(),
        };

        NavOutput { state: self.state, message: msg }
    }
}

// ----- Geometry -----

fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0_f64;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat/2.0).sin().powi(2) + lat1.to_radians().cos()*lat2.to_radians().cos()*(dlon/2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0-a).sqrt());
    r * c
}

// Corridor = within width/2 of any segment between consecutive waypoints.
// Distance is approximated by projecting to a local plane (good enough for small areas).
fn point_in_corridor(route: &RouteCfg, lat: f64, lon: f64) -> bool {
    let w = route.corridor_width_m / 2.0;
    if route.waypoints.len() < 2 { return false; }
    for seg in route.waypoints.windows(2) {
        let a = &seg[0]; let b = &seg[1];
        if dist_point_to_segment_m(lat, lon, a.lat, a.lon, b.lat, b.lon) <= w {
            return true;
        }
    }
    false
}

fn dist_point_to_segment_m(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    // simple equirectangular projection
    let (x, y) = to_xy(px, py, ax, ay);
    let (ax2, ay2) = (0.0, 0.0);
    let (bx2, by2) = to_xy(bx, by, ax, ay);

    let vx = bx2 - ax2; let vy = by2 - ay2;
    let wx = x - ax2; let wy = y - ay2;

    let c1 = wx*vx + wy*vy;
    if c1 <= 0.0 { return (wx*wx + wy*wy).sqrt(); }
    let c2 = vx*vx + vy*vy;
    if c2 <= c1 { return ((x-bx2).powi(2) + (y-by2).powi(2)).sqrt(); }
    let t = c1 / c2;
    let projx = ax2 + t*vx;
    let projy = ay2 + t*vy;
    ((x-projx).powi(2) + (y-projy).powi(2)).sqrt()
}

fn to_xy(lat: f64, lon: f64, lat0: f64, lon0: f64) -> (f64, f64) {
    let r = 6_371_000.0_f64;
    let x = (lon - lon0).to_radians() * r * lat0.to_radians().cos();
    let y = (lat - lat0).to_radians() * r;
    (x, y)
}

// Ray casting polygon test
fn point_in_polygon(poly: &[Point], lat: f64, lon: f64) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 { return false; }
    let mut j = n - 1;
    for i in 0..n {
        let xi = poly[i].lon; let yi = poly[i].lat;
        let xj = poly[j].lon; let yj = poly[j].lat;
        let intersect = ((yi > lat) != (yj > lat))
            && (lon < (xj - xi) * (lat - yi) / (yj - yi + 1e-12) + xi);
        if intersect { inside = !inside; }
        j = i;
    }
    inside
}
