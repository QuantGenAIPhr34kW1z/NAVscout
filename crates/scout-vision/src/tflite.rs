use anyhow::{Context, Result};
use image::{imageops::FilterType, DynamicImage};
use std::{ffi::CString, os::raw::{c_char, c_int, c_void}, ptr};
use tracing::info;

use crate::{Detection, VisionConfig, postprocess_ultralytics, nms_filter};

#[repr(C)]
struct TfLiteModel;
#[repr(C)]
struct TfLiteInterpreterOptions;
#[repr(C)]
struct TfLiteInterpreter;
#[repr(C)]
struct TfLiteTensor;
#[repr(C)]
struct TfLiteDelegate;

#[link(name = "tensorflowlite_c")]
extern "C" {
    fn TfLiteModelCreateFromFile(model_path: *const c_char) -> *mut TfLiteModel;
    fn TfLiteModelDelete(model: *mut TfLiteModel);

    fn TfLiteInterpreterOptionsCreate() -> *mut TfLiteInterpreterOptions;
    fn TfLiteInterpreterOptionsDelete(options: *mut TfLiteInterpreterOptions);
    fn TfLiteInterpreterOptionsSetNumThreads(options: *mut TfLiteInterpreterOptions, num_threads: c_int);
    fn TfLiteInterpreterOptionsAddDelegate(options: *mut TfLiteInterpreterOptions, delegate: *mut TfLiteDelegate);

    fn TfLiteInterpreterCreate(model: *const TfLiteModel, options: *const TfLiteInterpreterOptions) -> *mut TfLiteInterpreter;
    fn TfLiteInterpreterDelete(interpreter: *mut TfLiteInterpreter);

    fn TfLiteInterpreterAllocateTensors(interpreter: *mut TfLiteInterpreter) -> c_int;
    fn TfLiteInterpreterInvoke(interpreter: *mut TfLiteInterpreter) -> c_int;

    fn TfLiteInterpreterGetInputTensor(interpreter: *mut TfLiteInterpreter, index: c_int) -> *mut TfLiteTensor;
    fn TfLiteInterpreterGetOutputTensor(interpreter: *mut TfLiteInterpreter, index: c_int) -> *const TfLiteTensor;

    fn TfLiteTensorData(tensor: *const TfLiteTensor) -> *mut c_void;
    fn TfLiteTensorByteSize(tensor: *const TfLiteTensor) -> usize;

    fn TfLiteTensorNumDims(tensor: *const TfLiteTensor) -> c_int;
    fn TfLiteTensorDim(tensor: *const TfLiteTensor, dim_index: c_int) -> c_int;
}

#[cfg(feature = "vision-coral")]
#[link(name = "edgetpu")]
extern "C" {
    fn edgetpu_create_delegate(device_type: c_int, device_path: *const c_char, options: *const c_char) -> *mut TfLiteDelegate;
    fn edgetpu_free_delegate(delegate: *mut TfLiteDelegate);
}

pub struct TfliteDetector {
    cfg: VisionConfig,
    model: *mut TfLiteModel,
    opts: *mut TfLiteInterpreterOptions,
    interp: *mut TfLiteInterpreter,
    #[cfg(feature = "vision-coral")]
    delegate: Option<*mut TfLiteDelegate>,
}

unsafe impl Send for TfliteDetector {}
unsafe impl Sync for TfliteDetector {}

impl TfliteDetector {
    pub fn new(cfg: VisionConfig) -> Result<Self> {
        let model_path = if cfg.use_coral { &cfg.model_path_edgetpu } else { &cfg.model_path };
        let cpath = CString::new(model_path.as_str())?;
        let model = unsafe { TfLiteModelCreateFromFile(cpath.as_ptr()) };
        anyhow::ensure!(!model.is_null(), "failed to load tflite model: {}", model_path);

        let opts = unsafe { TfLiteInterpreterOptionsCreate() };
        anyhow::ensure!(!opts.is_null(), "failed to create tflite options");
        unsafe { TfLiteInterpreterOptionsSetNumThreads(opts, 2); } // conservative

        #[cfg(feature = "vision-coral")]
        let delegate = if cfg.use_coral {
            let d = unsafe { edgetpu_create_delegate(0, ptr::null(), ptr::null()) };
            anyhow::ensure!(!d.is_null(), "failed to create EdgeTPU delegate");
            unsafe { TfLiteInterpreterOptionsAddDelegate(opts, d); }
            Some(d)
        } else { None };

        #[cfg(not(feature = "vision-coral"))]
        if cfg.use_coral {
            anyhow::bail!("vision.use_coral=true but binary not built with --features vision-coral");
        }

        let interp = unsafe { TfLiteInterpreterCreate(model, opts) };
        anyhow::ensure!(!interp.is_null(), "failed to create tflite interpreter");

        let rc = unsafe { TfLiteInterpreterAllocateTensors(interp) };
        anyhow::ensure!(rc == 0, "TfLiteInterpreterAllocateTensors failed");

        info!("vision: loaded TFLite model: {}", model_path);

        Ok(Self {
            cfg, model, opts, interp,
            #[cfg(feature = "vision-coral")]
            delegate,
        })
    }

    pub fn inspect(&mut self) -> Result<String> {
        let input = unsafe { TfLiteInterpreterGetInputTensor(self.interp, 0) };
        anyhow::ensure!(!input.is_null(), "no input tensor");
        let in_dims = tensor_dims(input);
        let in_bytes = unsafe { TfLiteTensorByteSize(input) };

        let out0 = unsafe { TfLiteInterpreterGetOutputTensor(self.interp, 0) };
        anyhow::ensure!(!out0.is_null(), "no output tensor 0");
        let out_dims = tensor_dims(out0);
        let out_bytes = unsafe { TfLiteTensorByteSize(out0) };

        Ok(format!(
            "TFLite inspect:\n- input[0] dims={:?} bytes={}\n- output[0] dims={:?} bytes={}\n",
            in_dims, in_bytes, out_dims, out_bytes
        ))
    }

    pub fn detect_jpeg(&mut self, jpeg: &[u8]) -> Result<Vec<Detection>> {
        let img = image::load_from_memory(jpeg).context("decode jpeg")?;
        self.detect_image(img)
    }

    pub fn detect_jpeg_with_roi(&mut self, jpeg: &[u8], roi: Option<crate::Roi>) -> Result<Vec<Detection>> {
        let img = image::load_from_memory(jpeg).context("decode jpeg")?;
        if let Some(r) = roi {
            // Crop to ROI with margin, ensuring we stay in bounds
            let margin = self.cfg.roi_margin.unwrap_or(0.2);
            let w = img.width() as f32;
            let h = img.height() as f32;

            let roi_w = (r.w * (1.0 + margin)).min(1.0) * w;
            let roi_h = (r.h * (1.0 + margin)).min(1.0) * h;
            let roi_x = ((r.cx - r.w / 2.0 - roi_w / (2.0 * w)) * w).max(0.0).min(w - roi_w);
            let roi_y = ((r.cy - r.h / 2.0 - roi_h / (2.0 * h)) * h).max(0.0).min(h - roi_h);

            let cropped = img.crop_imm(
                roi_x as u32,
                roi_y as u32,
                roi_w as u32,
                roi_h as u32,
            );
            self.detect_image(cropped)
        } else {
            self.detect_image(img)
        }
    }

    fn detect_image(&mut self, img: DynamicImage) -> Result<Vec<Detection>> {
        let rgb = img.to_rgb8();
        let resized = image::imageops::resize(&rgb, self.cfg.img_w, self.cfg.img_h, FilterType::Triangle);

        // assumes u8 RGB input (quant/edgetpu-friendly)
        let input = unsafe { TfLiteInterpreterGetInputTensor(self.interp, 0) };
        anyhow::ensure!(!input.is_null(), "no input tensor");

        let in_bytes = unsafe { TfLiteTensorByteSize(input) };
        let in_ptr = unsafe { TfLiteTensorData(input) as *mut u8 };
        anyhow::ensure!(!in_ptr.is_null(), "null input tensor data");

        let need = (self.cfg.img_w * self.cfg.img_h * 3) as usize;
        anyhow::ensure!(in_bytes >= need, "input tensor too small: {} < {}", in_bytes, need);
        unsafe { ptr::copy_nonoverlapping(resized.as_raw().as_ptr(), in_ptr, need); }

        let rc = unsafe { TfLiteInterpreterInvoke(self.interp) };
        anyhow::ensure!(rc == 0, "TfLiteInterpreterInvoke failed");

        let out = unsafe { TfLiteInterpreterGetOutputTensor(self.interp, 0) };
        anyhow::ensure!(!out.is_null(), "no output tensor 0");

        let out_dims = tensor_dims(out);
        let (num_preds, stride) = match out_dims.as_slice() {
            [1, n, s] => (*n as usize, *s as usize),
            [n, s] => (*n as usize, *s as usize),
            other => anyhow::bail!(
                "unexpected output dims {:?}. Run `scout vision inspect` and set vision.output_layout accordingly.",
                other
            ),
        };

        let expected_stride = 5 + self.cfg.num_classes;
        anyhow::ensure!(
            stride == expected_stride,
            "stride mismatch: got {}, expected {}. output dims {:?}. You may need a different output_layout.",
            stride, expected_stride, out_dims
        );

        let out_ptr = unsafe { TfLiteTensorData(out) as *const f32 };
        anyhow::ensure!(!out_ptr.is_null(), "null output tensor data");
        let out_bytes = unsafe { TfLiteTensorByteSize(out) };
        let out_len = out_bytes / std::mem::size_of::<f32>();
        let raw = unsafe { std::slice::from_raw_parts(out_ptr, out_len) };

        let dets = match self.cfg.output_layout.as_str() {
            "ultralytics" => postprocess_ultralytics(raw, num_preds, self.cfg.num_classes, self.cfg.conf_threshold),
            other => anyhow::bail!(
                "unsupported output_layout: {} (dims={:?}). Run `scout vision inspect` to view tensors.",
                other, out_dims
            ),
        };

        Ok(nms_filter(dets, self.cfg.nms_iou_threshold, self.cfg.max_detections))
    }
}

fn tensor_dims(t: *const TfLiteTensor) -> Vec<i32> {
    unsafe {
        let nd = TfLiteTensorNumDims(t);
        let mut v = Vec::with_capacity(nd as usize);
        for i in 0..nd { v.push(TfLiteTensorDim(t, i)); }
        v
    }
}

impl Drop for TfliteDetector {
    fn drop(&mut self) {
        unsafe {
            if !self.interp.is_null() { TfLiteInterpreterDelete(self.interp); }
            if !self.opts.is_null() { TfLiteInterpreterOptionsDelete(self.opts); }
            if !self.model.is_null() { TfLiteModelDelete(self.model); }
        }
        #[cfg(feature = "vision-coral")]
        unsafe {
            if let Some(d) = self.delegate {
                edgetpu_free_delegate(d);
            }
        }
    }
}
