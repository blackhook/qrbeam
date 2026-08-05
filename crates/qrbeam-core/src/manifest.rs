use crate::constants::{
    MANIFEST_FRAGMENT_DATA_BYTES, MAX_FILE_BYTES, SEGMENT_BYTES_U32, SYMBOL_BYTES, SYMBOL_BYTES_U16,
};
use crate::error::ProtocolError;
use crate::frame::{Frame, FrameHeader, FrameType};

const MANIFEST_MAGIC: &[u8; 4] = b"QRMF";
const MANIFEST_FIXED_BYTES: usize = 112;
const MANIFEST_CRC_BYTES: usize = 4;
const PROFILE_BYTES: usize = 8;
const MAX_TEXT_BYTES: usize = 4_096;
const MAX_PROFILES: usize = 16;
const MAX_MANIFEST_FRAGMENTS: u32 = 512;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compression {
    None = 0,
    Gzip = 1,
}

impl TryFrom<u8> for Compression {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Gzip),
            other => Err(ProtocolError::UnknownCompression(other)),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EccLevel {
    L = 0,
    M = 1,
}

impl TryFrom<u8> for EccLevel {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::L),
            1 => Ok(Self::M),
            other => Err(ProtocolError::UnknownEccLevel(other)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Profile {
    pub id: u8,
    pub symbols_per_frame: u8,
    pub ecc: EccLevel,
    pub target_fps: u8,
    pub min_module_pixels: u8,
    pub max_qr_version: u8,
}

impl Profile {
    #[must_use]
    pub const fn defaults() -> [Self; 4] {
        [
            Self {
                id: 0,
                symbols_per_frame: 1,
                ecc: EccLevel::M,
                target_fps: 8,
                min_module_pixels: 6,
                max_qr_version: 40,
            },
            Self {
                id: 1,
                symbols_per_frame: 5,
                ecc: EccLevel::L,
                target_fps: 24,
                min_module_pixels: 6,
                max_qr_version: 40,
            },
            Self {
                id: 2,
                symbols_per_frame: 8,
                ecc: EccLevel::L,
                target_fps: 30,
                min_module_pixels: 4,
                max_qr_version: 40,
            },
            Self {
                id: 3,
                symbols_per_frame: 10,
                ecc: EccLevel::L,
                target_fps: 60,
                min_module_pixels: 4,
                max_qr_version: 40,
            },
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub session_id: [u8; 16],
    pub file_id: u32,
    pub filename: String,
    pub mime_type: String,
    pub original_length: u64,
    pub container_length: u64,
    pub compression: Compression,
    pub file_hash: [u8; 32],
    pub segment_size: u32,
    pub symbol_size: u16,
    pub last_segment_length: u32,
    pub encoding_seed: [u8; 16],
    pub profiles: Vec<Profile>,
    pub segment_crc32c: Vec<u32>,
}

impl Manifest {
    /// Encodes the manifest using the deterministic `QRBeam` v1 field order.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when a size, count, profile, or text field is
    /// outside the v1 protocol bounds.
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let filename = self.filename.as_bytes();
        let mime_type = self.mime_type.as_bytes();
        let filename_length = u16::try_from(filename.len())
            .map_err(|_| ProtocolError::InvalidManifest("filename is too long"))?;
        let mime_length = u16::try_from(mime_type.len())
            .map_err(|_| ProtocolError::InvalidManifest("MIME type is too long"))?;
        let profile_count = u8::try_from(self.profiles.len())
            .map_err(|_| ProtocolError::InvalidManifest("too many profiles"))?;
        let segment_count = u32::try_from(self.segment_crc32c.len())
            .map_err(|_| ProtocolError::InvalidManifest("too many segments"))?;

        let variable_length = filename
            .len()
            .checked_add(mime_type.len())
            .and_then(|value| value.checked_add(self.profiles.len() * PROFILE_BYTES))
            .and_then(|value| value.checked_add(self.segment_crc32c.len() * 4))
            .ok_or(ProtocolError::InvalidManifest("encoded length overflows"))?;
        let mut bytes =
            Vec::with_capacity(MANIFEST_FIXED_BYTES + variable_length + MANIFEST_CRC_BYTES);
        bytes.extend_from_slice(MANIFEST_MAGIC);
        bytes.push(crate::constants::PROTOCOL_VERSION);
        bytes.push(self.compression as u8);
        bytes.extend_from_slice(&[0; 2]);
        bytes.extend_from_slice(&self.session_id);
        bytes.extend_from_slice(&self.file_id.to_le_bytes());
        bytes.extend_from_slice(&self.original_length.to_le_bytes());
        bytes.extend_from_slice(&self.container_length.to_le_bytes());
        bytes.extend_from_slice(&self.file_hash);
        bytes.extend_from_slice(&self.segment_size.to_le_bytes());
        bytes.extend_from_slice(&self.symbol_size.to_le_bytes());
        bytes.push(profile_count);
        bytes.push(0);
        bytes.extend_from_slice(&self.last_segment_length.to_le_bytes());
        bytes.extend_from_slice(&self.encoding_seed);
        bytes.extend_from_slice(&filename_length.to_le_bytes());
        bytes.extend_from_slice(&mime_length.to_le_bytes());
        bytes.extend_from_slice(&segment_count.to_le_bytes());
        debug_assert_eq!(bytes.len(), MANIFEST_FIXED_BYTES);
        bytes.extend_from_slice(filename);
        bytes.extend_from_slice(mime_type);
        for profile in &self.profiles {
            encode_profile(&mut bytes, *profile);
        }
        for checksum in &self.segment_crc32c {
            bytes.extend_from_slice(&checksum.to_le_bytes());
        }
        let crc = crc32c::crc32c(&bytes);
        bytes.extend_from_slice(&crc.to_le_bytes());
        Ok(bytes)
    }

    /// Decodes and validates one complete deterministic manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when CRC, UTF-8, lengths, reserved bytes, or
    /// semantic file and segment bounds are invalid.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let minimum = MANIFEST_FIXED_BYTES + MANIFEST_CRC_BYTES;
        if bytes.len() < minimum {
            return Err(ProtocolError::ManifestTooShort {
                actual: bytes.len(),
                minimum,
            });
        }
        let crc_offset = bytes.len() - MANIFEST_CRC_BYTES;
        let expected_crc = read_u32(bytes, crc_offset);
        let actual_crc = crc32c::crc32c(&bytes[..crc_offset]);
        if expected_crc != actual_crc {
            return Err(ProtocolError::ManifestCrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }
        if &bytes[0..4] != MANIFEST_MAGIC {
            return Err(ProtocolError::InvalidManifest("magic does not match"));
        }
        if bytes[4] != crate::constants::PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(bytes[4]));
        }
        if bytes[6] != 0 || bytes[7] != 0 || bytes[83] != 0 {
            return Err(ProtocolError::InvalidManifest(
                "reserved fields must be zero",
            ));
        }

        let filename_length = usize::from(read_u16(bytes, 104));
        let mime_length = usize::from(read_u16(bytes, 106));
        let profile_count = usize::from(bytes[82]);
        let segment_count = usize::try_from(read_u32(bytes, 108))
            .map_err(|_| ProtocolError::InvalidManifest("segment count is too large"))?;
        let required_length = MANIFEST_FIXED_BYTES
            .checked_add(filename_length)
            .and_then(|value| value.checked_add(mime_length))
            .and_then(|value| value.checked_add(profile_count * PROFILE_BYTES))
            .and_then(|value| value.checked_add(segment_count * 4))
            .and_then(|value| value.checked_add(MANIFEST_CRC_BYTES))
            .ok_or(ProtocolError::InvalidManifest("declared length overflows"))?;
        if required_length != bytes.len() {
            return Err(ProtocolError::InvalidManifest(
                "declared fields do not match encoded length",
            ));
        }

        let mut session_id = [0_u8; 16];
        session_id.copy_from_slice(&bytes[8..24]);
        let mut file_hash = [0_u8; 32];
        file_hash.copy_from_slice(&bytes[44..76]);
        let mut encoding_seed = [0_u8; 16];
        encoding_seed.copy_from_slice(&bytes[88..104]);

        let mut cursor = MANIFEST_FIXED_BYTES;
        let filename_end = cursor + filename_length;
        let filename = std::str::from_utf8(&bytes[cursor..filename_end])
            .map_err(|_| ProtocolError::InvalidManifestUtf8)?
            .to_owned();
        cursor = filename_end;
        let mime_end = cursor + mime_length;
        let mime_type = std::str::from_utf8(&bytes[cursor..mime_end])
            .map_err(|_| ProtocolError::InvalidManifestUtf8)?
            .to_owned();
        cursor = mime_end;

        let mut profiles = Vec::with_capacity(profile_count);
        for _ in 0..profile_count {
            profiles.push(decode_profile(&bytes[cursor..cursor + PROFILE_BYTES])?);
            cursor += PROFILE_BYTES;
        }
        let mut segment_crc32c = Vec::with_capacity(segment_count);
        for _ in 0..segment_count {
            segment_crc32c.push(read_u32(bytes, cursor));
            cursor += 4;
        }
        debug_assert_eq!(cursor, crc_offset);

        let manifest = Self {
            session_id,
            file_id: read_u32(bytes, 24),
            filename,
            mime_type,
            original_length: read_u64(bytes, 28),
            container_length: read_u64(bytes, 36),
            compression: Compression::try_from(bytes[5])?,
            file_hash,
            segment_size: read_u32(bytes, 76),
            symbol_size: read_u16(bytes, 80),
            last_segment_length: read_u32(bytes, 84),
            encoding_seed,
            profiles,
            segment_crc32c,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Splits the encoded manifest into stable-channel payload fragments.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the manifest itself cannot be encoded.
    pub fn fragments(&self) -> Result<Vec<ManifestFragment>, ProtocolError> {
        let encoded = self.encode()?;
        let manifest_hash = *blake3::hash(&encoded).as_bytes();
        let count = u32::try_from(encoded.len().div_ceil(MANIFEST_FRAGMENT_DATA_BYTES))
            .map_err(|_| ProtocolError::InvalidManifest("too many manifest fragments"))?;
        if count == 0 || count > MAX_MANIFEST_FRAGMENTS {
            return Err(ProtocolError::InvalidManifest(
                "manifest fragment count is outside protocol bounds",
            ));
        }
        Ok((0_u32..)
            .zip(encoded.chunks(MANIFEST_FRAGMENT_DATA_BYTES))
            .map(|(index, payload)| ManifestFragment {
                index,
                count,
                manifest_hash,
                payload: payload.to_vec(),
            })
            .collect())
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if self.original_length > MAX_FILE_BYTES {
            return Err(ProtocolError::FileTooLarge {
                actual: self.original_length,
                maximum: MAX_FILE_BYTES,
            });
        }
        if self.container_length > MAX_FILE_BYTES {
            return Err(ProtocolError::FileTooLarge {
                actual: self.container_length,
                maximum: MAX_FILE_BYTES,
            });
        }
        if self.segment_size != SEGMENT_BYTES_U32 {
            return Err(ProtocolError::InvalidManifest("unsupported segment size"));
        }
        if self.symbol_size != SYMBOL_BYTES_U16 {
            return Err(ProtocolError::InvalidManifest("unsupported symbol size"));
        }
        if self.filename.is_empty() || self.filename.len() > MAX_TEXT_BYTES {
            return Err(ProtocolError::InvalidManifest("invalid filename length"));
        }
        if self.mime_type.len() > MAX_TEXT_BYTES {
            return Err(ProtocolError::InvalidManifest("invalid MIME type length"));
        }
        if self.profiles.is_empty() || self.profiles.len() > MAX_PROFILES {
            return Err(ProtocolError::InvalidManifest("invalid profile count"));
        }
        for profile in &self.profiles {
            validate_profile(*profile)?;
        }

        let expected_segments = if self.container_length == 0 {
            0
        } else {
            self.container_length.div_ceil(u64::from(SEGMENT_BYTES_U32))
        };
        if self.segment_crc32c.len()
            != usize::try_from(expected_segments)
                .map_err(|_| ProtocolError::InvalidManifest("segment count is too large"))?
        {
            return Err(ProtocolError::InvalidManifest(
                "segment checksums do not match container length",
            ));
        }
        let expected_last_length = if self.container_length == 0 {
            0
        } else {
            let remainder = self.container_length % u64::from(SEGMENT_BYTES_U32);
            if remainder == 0 {
                u64::from(SEGMENT_BYTES_U32)
            } else {
                remainder
            }
        };
        if u64::from(self.last_segment_length) != expected_last_length {
            return Err(ProtocolError::InvalidManifest(
                "last segment length does not match container length",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestFragment {
    pub index: u32,
    pub count: u32,
    pub manifest_hash: [u8; 32],
    pub payload: Vec<u8>,
}

impl ManifestFragment {
    /// Wraps this fragment in a stable-channel manifest frame.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when indices or payload size are invalid.
    pub fn into_frame(
        self,
        session_id: [u8; 16],
        file_id: u32,
        global_frame_index: u64,
        channel_id: u8,
        profile_id: u8,
    ) -> Result<Frame, ProtocolError> {
        if self.count == 0 || self.count > MAX_MANIFEST_FRAGMENTS || self.index >= self.count {
            return Err(ProtocolError::InvalidManifestFragment(
                "fragment index or count is invalid",
            ));
        }
        if self.payload.is_empty() || self.payload.len() > MANIFEST_FRAGMENT_DATA_BYTES {
            return Err(ProtocolError::InvalidManifestFragment(
                "fragment payload length is invalid",
            ));
        }
        let mut payload = Vec::with_capacity(32 + self.payload.len());
        payload.extend_from_slice(&self.manifest_hash);
        payload.extend_from_slice(&self.payload);
        Ok(Frame {
            header: FrameHeader {
                frame_type: FrameType::Manifest,
                flags: u8::from(self.index + 1 == self.count),
                channel_id,
                profile_id,
                session_id,
                file_id,
                global_frame_index,
                segment_index: self.index,
                first_symbol_id: self.count,
                symbol_count: 0,
            },
            payload,
        })
    }
}

#[derive(Debug, Default)]
pub struct ManifestAssembler {
    manifest_hash: Option<[u8; 32]>,
    fragment_count: Option<u32>,
    fragments: Vec<Option<Vec<u8>>>,
    received: u32,
    session_id: Option<[u8; 16]>,
    file_id: Option<u32>,
    completed: bool,
}

impl ManifestAssembler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one already frame-validated manifest fragment.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when fragment metadata conflicts, the full
    /// BLAKE3 is wrong, or the assembled manifest is invalid.
    pub fn push(&mut self, frame: &Frame) -> Result<Option<Manifest>, ProtocolError> {
        if frame.header.frame_type != FrameType::Manifest {
            return Err(ProtocolError::InvalidManifestFragment(
                "frame type is not manifest",
            ));
        }
        if frame.payload.len() <= 32 || frame.payload.len() > SYMBOL_BYTES {
            return Err(ProtocolError::InvalidManifestFragment(
                "frame payload length is invalid",
            ));
        }
        let count = frame.header.first_symbol_id;
        let index = frame.header.segment_index;
        if count == 0 || count > MAX_MANIFEST_FRAGMENTS || index >= count {
            return Err(ProtocolError::InvalidManifestFragment(
                "fragment index or count is invalid",
            ));
        }
        let mut hash = [0_u8; 32];
        hash.copy_from_slice(&frame.payload[..32]);
        let fragment_payload = &frame.payload[32..];
        if index + 1 < count && fragment_payload.len() != MANIFEST_FRAGMENT_DATA_BYTES {
            return Err(ProtocolError::InvalidManifestFragment(
                "non-final fragment has the wrong length",
            ));
        }

        if self.fragment_count.is_none() {
            self.fragment_count = Some(count);
            self.manifest_hash = Some(hash);
            self.session_id = Some(frame.header.session_id);
            self.file_id = Some(frame.header.file_id);
            self.fragments = vec![None; count as usize];
        } else if self.fragment_count != Some(count)
            || self.manifest_hash != Some(hash)
            || self.session_id != Some(frame.header.session_id)
            || self.file_id != Some(frame.header.file_id)
        {
            return Err(ProtocolError::ManifestFragmentConflict);
        }

        let slot = &mut self.fragments[index as usize];
        if let Some(existing) = slot {
            if existing == fragment_payload {
                return Ok(None);
            }
            return Err(ProtocolError::ManifestFragmentConflict);
        }
        *slot = Some(fragment_payload.to_vec());
        self.received += 1;
        if self.received != count || self.completed {
            return Ok(None);
        }

        let encoded: Vec<u8> = self
            .fragments
            .iter()
            .flatten()
            .flat_map(|fragment| fragment.iter().copied())
            .collect();
        if blake3::hash(&encoded).as_bytes() != &hash {
            return Err(ProtocolError::ManifestHashMismatch);
        }
        let manifest = Manifest::decode(&encoded)?;
        if self.session_id != Some(manifest.session_id) || self.file_id != Some(manifest.file_id) {
            return Err(ProtocolError::ManifestFragmentConflict);
        }
        self.completed = true;
        Ok(Some(manifest))
    }

    #[must_use]
    pub const fn received_count(&self) -> u32 {
        self.received
    }
}

fn encode_profile(bytes: &mut Vec<u8>, profile: Profile) {
    bytes.extend_from_slice(&[
        profile.id,
        profile.symbols_per_frame,
        profile.ecc as u8,
        profile.target_fps,
        profile.min_module_pixels,
        profile.max_qr_version,
        0,
        0,
    ]);
}

fn decode_profile(bytes: &[u8]) -> Result<Profile, ProtocolError> {
    if bytes[6] != 0 || bytes[7] != 0 {
        return Err(ProtocolError::InvalidManifest(
            "profile reserved bytes must be zero",
        ));
    }
    let profile = Profile {
        id: bytes[0],
        symbols_per_frame: bytes[1],
        ecc: EccLevel::try_from(bytes[2])?,
        target_fps: bytes[3],
        min_module_pixels: bytes[4],
        max_qr_version: bytes[5],
    };
    validate_profile(profile)?;
    Ok(profile)
}

fn validate_profile(profile: Profile) -> Result<(), ProtocolError> {
    if profile.symbols_per_frame == 0 || profile.symbols_per_frame > 10 {
        return Err(ProtocolError::InvalidManifest(
            "profile symbol count is outside protocol bounds",
        ));
    }
    if profile.target_fps == 0 || profile.target_fps > 60 {
        return Err(ProtocolError::InvalidManifest(
            "profile FPS is outside protocol bounds",
        ));
    }
    if !(1..=40).contains(&profile.max_qr_version) {
        return Err(ProtocolError::InvalidManifest(
            "profile QR version is outside protocol bounds",
        ));
    }
    if profile.min_module_pixels == 0 {
        return Err(ProtocolError::InvalidManifest(
            "profile module size must be positive",
        ));
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}
