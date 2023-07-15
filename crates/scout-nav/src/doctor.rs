use anyhow::Result;
use crate::nav::{Home, RouteCfg, ZoneCfg};

pub fn check_gnss_thresholds(min_sats: u8, max_hdop: f32, max_fix_age_s: u64) -> Result<()> {
    anyhow::ensure!(min_sats >= 4, "gnss.min_sats too low");
    anyhow::ensure!(max_hdop > 0.5 && max_hdop < 5.0, "gnss.max_hdop out of range");
    anyhow::ensure!(max_fix_age_s >= 1 && max_fix_age_s <= 10, "gnss.max_fix_age_s should be 1..10");
    Ok(())
}

pub fn check_geofence(home: &Home, route: &RouteCfg, zone: &ZoneCfg, max_radius_m: f64) -> Result<()> {
    anyhow::ensure!(route.waypoints.len() >= 2, "nav.route.waypoints must have >= 2 points");
    anyhow::ensure!(zone.zone_polygon.len() >= 3, "nav.zone.zone_polygon must have >= 3 points");
    anyhow::ensure!(max_radius_m >= 50.0, "nav.max_radius_m too small");
    // Basic sanity: home not NaN
    anyhow::ensure!(home.lat.abs() <= 90.0 && home.lon.abs() <= 180.0, "home coordinates invalid");
    Ok(())
}
