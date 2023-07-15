use crate::Detection;

#[derive(Debug, Clone)]
pub struct TrackingConfig {
    pub enable: bool,
    pub max_age_frames: u32,
    pub min_hits: u32,
    pub iou_match_threshold: f32,
    pub max_tracks: usize,
    pub target_class: String,
    pub lock_min_conf: f32,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: u64,
    pub class_id: i32,
    pub conf: f32,
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,

    // velocity (simple constant-velocity model)
    pub vx: f32,
    pub vy: f32,

    pub hits: u32,
    pub age: u32,       // frames since created
    pub miss: u32,      // frames since last match
}

#[derive(Debug, Clone)]
pub struct Tracker {
    cfg: TrackingConfig,
    next_id: u64,
    tracks: Vec<Track>,
    locked_id: Option<u64>,
    target_class_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct TrackOutput {
    pub tracks: Vec<Track>,
    pub locked: Option<Track>,
    pub note: String,
}

impl Tracker {
    pub fn new(cfg: TrackingConfig, class_names: &[String]) -> Self {
        let target_class_id = class_names.iter().position(|c| c == &cfg.target_class).map(|i| i as i32);
        Self { cfg, next_id: 1, tracks: vec![], locked_id: None, target_class_id }
    }

    pub fn has_lock(&self) -> bool {
        self.locked_id.is_some()
    }

    pub fn update(&mut self, dets: &[Detection]) -> TrackOutput {
        if !self.cfg.enable {
            return TrackOutput { tracks: vec![], locked: None, note: "tracking disabled".into() };
        }

        // Predict step: advance tracks by velocity and decay confidence a bit
        for t in &mut self.tracks {
            t.cx = (t.cx + t.vx).clamp(0.0, 1.0);
            t.cy = (t.cy + t.vy).clamp(0.0, 1.0);
            t.age += 1;
            t.miss += 1;
            t.conf *= 0.995;
        }

        // Greedy association by IOU
        let mut used_det = vec![false; dets.len()];
        for t in &mut self.tracks {
            let mut best_i = None;
            let mut best_iou = 0.0;
            for (i, d) in dets.iter().enumerate() {
                if used_det[i] { continue; }
                if d.class_id != t.class_id { continue; }
                let iou = iou(t.cx,t.cy,t.w,t.h, d.cx,d.cy,d.w,d.h);
                if iou > best_iou {
                    best_iou = iou;
                    best_i = Some(i);
                }
            }
            if let Some(i) = best_i {
                if best_iou >= self.cfg.iou_match_threshold {
                    let d = &dets[i];
                    used_det[i] = true;

                    // update velocity estimate (simple)
                    let nx = d.cx - t.cx;
                    let ny = d.cy - t.cy;
                    t.vx = 0.7*t.vx + 0.3*nx;
                    t.vy = 0.7*t.vy + 0.3*ny;

                    t.cx = d.cx; t.cy = d.cy;
                    t.w = d.w; t.h = d.h;
                    t.conf = d.conf.max(t.conf);
                    t.hits += 1;
                    t.miss = 0;
                }
            }
        }

        // Create new tracks for unmatched detections
        for (i, d) in dets.iter().enumerate() {
            if used_det[i] { continue; }
            if self.tracks.len() >= self.cfg.max_tracks { break; }
            self.tracks.push(Track {
                id: self.next_id,
                class_id: d.class_id,
                conf: d.conf,
                cx: d.cx, cy: d.cy, w: d.w, h: d.h,
                vx: 0.0, vy: 0.0,
                hits: 1, age: 1, miss: 0,
            });
            self.next_id += 1;
        }

        // Prune old tracks
        self.tracks.retain(|t| t.miss <= self.cfg.max_age_frames);

        // Lock policy:
        // - prefer existing lock if still alive
        // - otherwise pick best target-class track with enough conf
        let mut note = String::new();
        if let Some(id) = self.locked_id {
            if self.tracks.iter().any(|t| t.id == id) {
                note = format!("lock kept: {}", id);
            } else {
                self.locked_id = None;
                note = "lock lost".into();
            }
        }

        if self.locked_id.is_none() {
            if let Some(tc) = self.target_class_id {
                let mut best: Option<&Track> = None;
                for t in &self.tracks {
                    if t.class_id != tc { continue; }
                    if t.conf < self.cfg.lock_min_conf { continue; }
                    if t.hits < self.cfg.min_hits { continue; }
                    best = match best {
                        None => Some(t),
                        Some(b) => if t.conf > b.conf { Some(t) } else { Some(b) },
                    };
                }
                if let Some(b) = best {
                    self.locked_id = Some(b.id);
                    note = format!("lock acquired: {}", b.id);
                }
            }
        }

        let locked = self.locked_id.and_then(|id| self.tracks.iter().find(|t| t.id == id)).cloned();
        TrackOutput { tracks: self.tracks.clone(), locked, note }
    }
}

fn iou(cx1: f32, cy1: f32, w1: f32, h1: f32, cx2: f32, cy2: f32, w2: f32, h2: f32) -> f32 {
    let (x1a,y1a,x1b,y1b) = (cx1-w1/2.0, cy1-h1/2.0, cx1+w1/2.0, cy1+h1/2.0);
    let (x2a,y2a,x2b,y2b) = (cx2-w2/2.0, cy2-h2/2.0, cx2+w2/2.0, cy2+h2/2.0);
    let ix_a = x1a.max(x2a);
    let iy_a = y1a.max(y2a);
    let ix_b = x1b.min(x2b);
    let iy_b = y1b.min(y2b);
    let iw = (ix_b - ix_a).max(0.0);
    let ih = (iy_b - iy_a).max(0.0);
    let inter = iw*ih;
    let a1 = (x1b-x1a).max(0.0) * (y1b-y1a).max(0.0);
    let a2 = (x2b-x2a).max(0.0) * (y2b-y2a).max(0.0);
    let u = a1 + a2 - inter;
    if u <= 0.0 { 0.0 } else { inter / u }
}
