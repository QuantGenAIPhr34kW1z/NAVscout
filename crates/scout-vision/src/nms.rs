pub fn iou(cx1: f32, cy1: f32, w1: f32, h1: f32, cx2: f32, cy2: f32, w2: f32, h2: f32) -> f32 {
    let (x1a, y1a, x1b, y1b) = (cx1 - w1/2.0, cy1 - h1/2.0, cx1 + w1/2.0, cy1 + h1/2.0);
    let (x2a, y2a, x2b, y2b) = (cx2 - w2/2.0, cy2 - h2/2.0, cx2 + w2/2.0, cy2 + h2/2.0);

    let ix_a = x1a.max(x2a);
    let iy_a = y1a.max(y2a);
    let ix_b = x1b.min(x2b);
    let iy_b = y1b.min(y2b);

    let iw = (ix_b - ix_a).max(0.0);
    let ih = (iy_b - iy_a).max(0.0);
    let inter = iw * ih;
    let a1 = (x1b - x1a).max(0.0) * (y1b - y1a).max(0.0);
    let a2 = (x2b - x2a).max(0.0) * (y2b - y2a).max(0.0);
    let union = a1 + a2 - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}
