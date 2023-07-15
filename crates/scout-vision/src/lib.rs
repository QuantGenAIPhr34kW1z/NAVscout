mod nms;
pub mod camera;
pub mod tflite;
pub mod tracker;
pub mod power;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct Roi {
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
}

impl Roi {
    pub fn clamp01(self) -> Self {
        Self {
            cx: self.cx.clamp(0.0, 1.0),
            cy: self.cy.clamp(0.0, 1.0),
            w: self.w.clamp(0.0, 1.0),
            h: self.h.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    pub class_id: i32,
    pub conf: f32,
    // normalized 0..1
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VisionConfig {
    pub enable: bool,
    pub backend: String, // "tflite"
    pub use_coral: bool,
    pub model_path: String,
    pub model_path_edgetpu: String,

    pub img_w: u32,
    pub img_h: u32,
    pub num_classes: usize,
    pub class_names: Vec<String>,

    pub conf_threshold: f32,
    pub nms_iou_threshold: f32,
    pub max_detections: usize,
    pub output_layout: String, // "ultralytics"

    pub roi_enable: Option<bool>,
    pub roi_margin: Option<f32>,
    pub roi_min_size: Option<f32>,
}

pub trait Detector: Send + Sync {
    fn detect_rgb(&mut self, rgb: &[u8], w: u32, h: u32) -> Result<Vec<Detection>>;
}

pub fn postprocess_ultralytics(
    raw: &[f32],
    num_preds: usize,
    num_classes: usize,
    conf_th: f32,
) -> Vec<Detection> {
    // Ultralytics common export:
    // [cx, cy, w, h, obj, cls0..]
    let stride = 5 + num_classes;
    let mut out = Vec::new();

    for i in 0..num_preds {
        let base = i * stride;
        if base + stride > raw.len() { break; }
        let cx = raw[base + 0];
        let cy = raw[base + 1];
        let w = raw[base + 2];
        let h = raw[base + 3];
        let obj = raw[base + 4];

        let mut best_c = 0usize;
        let mut best_p = 0.0f32;
        for c in 0..num_classes {
            let p = raw[base + 5 + c];
            if p > best_p { best_p = p; best_c = c; }
        }
        let conf = obj * best_p;
        if conf >= conf_th {
            out.push(Detection { class_id: best_c as i32, conf, cx, cy, w, h });
        }
    }
    out
}

pub fn nms_filter(mut dets: Vec<Detection>, iou_th: f32, max_det: usize) -> Vec<Detection> {
    dets.sort_by(|a, b| b.conf.partial_cmp(&a.conf).unwrap_or(std::cmp::Ordering::Equal));
    let mut kept: Vec<Detection> = Vec::new();

    'outer: for d in dets {
        for k in &kept {
            if nms::iou(d.cx, d.cy, d.w, d.h, k.cx, k.cy, k.w, k.h) >= iou_th {
                continue 'outer;
            }
        }
        kept.push(d);
        if kept.len() >= max_det { break; }
    }
    kept
}
