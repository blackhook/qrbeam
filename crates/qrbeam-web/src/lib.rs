use qrbeam_core::manifest::{Manifest, ManifestAssembler};
use qrbeam_core::persistent_receiver::{PersistentReceiver, PersistentUpdate};
use qrbeam_core::session::SendSession;
use qrbeam_core::timeline::ChannelRequest;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const TURBO_CHANNEL: ChannelRequest = ChannelRequest {
    channel_id: 0,
    profile_id: 3,
    symbols_per_frame: 11,
};

#[derive(Deserialize)]
struct SenderInput {
    filename: String,
    mime_type: String,
    data: Vec<u8>,
    session_id: Vec<u8>,
    file_id: u32,
}

#[wasm_bindgen]
pub struct WebSender {
    inner: SendSession,
}

#[wasm_bindgen]
impl WebSender {
    #[wasm_bindgen(constructor)]
    pub fn new(input: JsValue) -> Result<Self, JsValue> {
        let input: SenderInput = serde_wasm_bindgen::from_value(input).map_err(js_error)?;
        let session_id: [u8; 16] = input
            .session_id
            .try_into()
            .map_err(|_| JsValue::from_str("session_id must contain 16 bytes"))?;
        SendSession::new(
            &input.filename,
            &input.mime_type,
            &input.data,
            session_id,
            input.file_id,
        )
        .map(|inner| Self { inner })
        .map_err(js_error)
    }

    pub fn manifest_frames(&self) -> Result<JsValue, JsValue> {
        let manifest = self.inner.manifest();
        let frames: Result<Vec<Vec<u8>>, _> = manifest
            .fragments()
            .map_err(js_error)?
            .into_iter()
            .enumerate()
            .map(|(index, fragment)| {
                fragment
                    .into_frame(
                        manifest.session_id,
                        manifest.file_id,
                        u64::try_from(index).unwrap_or(u64::MAX),
                        0,
                        0,
                    )?
                    .encode()
            })
            .collect();
        serde_wasm_bindgen::to_value(&frames.map_err(js_error)?).map_err(js_error)
    }

    pub fn next_turbo_frame(&mut self) -> Result<Vec<u8>, JsValue> {
        self.inner
            .next_frames(&[TURBO_CHANNEL])
            .map_err(js_error)
            .and_then(|mut frames| {
                frames
                    .pop()
                    .ok_or_else(|| JsValue::from_str("missing frame"))
            })
    }
}

#[wasm_bindgen]
pub struct WebReceiver {
    inner: PersistentReceiver,
}

#[derive(Serialize)]
struct ReceiverEvent {
    kind: String,
    index: Option<u32>,
    bytes: Option<Vec<u8>>,
}

#[wasm_bindgen]
pub struct WebManifestAssembler {
    inner: ManifestAssembler,
}

impl Default for WebManifestAssembler {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WebManifestAssembler {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ManifestAssembler::new(),
        }
    }

    pub fn push(&mut self, frame: &[u8]) -> Result<JsValue, JsValue> {
        let frame = qrbeam_core::frame::Frame::decode(frame).map_err(js_error)?;
        let manifest = self.inner.push(&frame).map_err(js_error)?;
        let encoded = manifest
            .map(|manifest| manifest.encode())
            .transpose()
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&encoded).map_err(js_error)
    }
}

#[wasm_bindgen]
impl WebReceiver {
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: &[u8]) -> Result<Self, JsValue> {
        Manifest::decode(manifest)
            .and_then(PersistentReceiver::from_manifest)
            .map(|inner| Self { inner })
            .map_err(js_error)
    }

    pub fn ingest(&mut self, frame: &[u8]) -> Result<JsValue, JsValue> {
        let event = match self.inner.ingest(frame).map_err(js_error)? {
            PersistentUpdate::Accepted => ReceiverEvent {
                kind: "accepted".to_owned(),
                index: None,
                bytes: None,
            },
            PersistentUpdate::IgnoredDuplicate => ReceiverEvent {
                kind: "duplicate".to_owned(),
                index: None,
                bytes: None,
            },
            PersistentUpdate::ManifestRefreshed => ReceiverEvent {
                kind: "manifest-refreshed".to_owned(),
                index: None,
                bytes: None,
            },
            PersistentUpdate::SegmentReady { index, bytes } => ReceiverEvent {
                kind: "segment-ready".to_owned(),
                index: Some(index),
                bytes: Some(bytes),
            },
        };
        serde_wasm_bindgen::to_value(&event).map_err(js_error)
    }

    pub fn acknowledge_segment(&mut self, index: u32) -> Result<(), JsValue> {
        self.inner.acknowledge_segment(index).map_err(js_error)
    }

    pub fn snapshot_json(&self) -> String {
        format!("{:?}", self.inner.snapshot())
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
