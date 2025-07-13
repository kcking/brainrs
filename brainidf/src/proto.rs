/*
    enum class Encoding : uint8_t {
        DIRECT_ARGB = 0,
        DIRECT_RGB,
        INDEXED_2,
        INDEXED_4,
        INDEXED_16
    };
*/

use std::io::{Read, Write};

#[repr(u8)]
pub enum PixelShaderEncoding {
    DirectArgb = 0,
    DirectRgb,
    Indexed2,
    Indexed4,
    Indexed16,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum MessageType {
    BrainHello = 0u8,
    BrainPanelShade = 1u8,
}

#[derive(Debug, Clone)]
pub struct Header {
    id: i16,
    frame_size: i16,
    msg_size: i32,
    frame_offset: i32,
}

pub const FRAGMENT_MAX: usize = 1500;
pub const HEADER_SIZE: usize = 12;

impl Header {
    pub fn from_payload(id: i16, msg: &[u8]) -> Self {
        // TODO: impl fragmentation
        Self {
            id,
            frame_size: msg.len() as i16,
            msg_size: msg.len() as i32,
            frame_offset: 0,
        }
    }
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        let mut w = buf.as_mut_slice();
        w.write_all(&self.id.to_be_bytes()).unwrap();
        w.write_all(&self.frame_size.to_be_bytes()).unwrap();
        w.write_all(&self.msg_size.to_be_bytes()).unwrap();
        w.write_all(&self.frame_offset.to_be_bytes()).unwrap();
        buf
    }
    pub fn from_reader(r: &mut impl Read) -> Self {
        let mut id_bytes = [0u8; 2];
        r.read_exact(&mut id_bytes).unwrap();
        let mut frame_size = [0u8; 2];
        r.read_exact(&mut frame_size).unwrap();
        let mut msg_size = [0u8; size_of::<i32>()];
        r.read_exact(&mut msg_size).unwrap();
        let mut frame_offset = [0u8; size_of::<i32>()];
        r.read_exact(&mut frame_offset).unwrap();
        Self {
            id: i16::from_be_bytes(id_bytes),
            frame_size: i16::from_be_bytes(frame_size),
            msg_size: i32::from_be_bytes(msg_size),
            frame_offset: i32::from_be_bytes(frame_offset),
        }
    }
}

/*
    enum class Type : uint8_t {
        BRAIN_HELLO,       // Brain -> Pinky|Mapper
        BRAIN_PANEL_SHADE, // Pinky -> Brain
        MAPPER_HELLO,      // Mapper -> Pinky
        BRAIN_ID_REQUEST,  // Mapper -> Brain
        BRAIN_MAPPING,
        PING,
        USE_FIRMWARE,
    };

    static const size_t FRAGMENT_MAX = 1500;
    static const size_t HEADER_SIZE = 12;

    struct Header {
        int16_t id;
        int16_t frameSize;
        int32_t msgSize;
        int32_t frameOffset;
    };

    BrainHelloMsg(const char *brainId,
            const char *panelName,
            const char *firmwareVersion,
            const char *idfVersion) {
        // Need capacity for:
        //      id byte
        //      brainId string
        //      panelName NullableString (adds 1 byte boolean)
        //      firmwareVersion string
        //      idfVersion string
        if (prepCapacity(
                1 +
                capFor(brainId) +
                capForNullable(panelName) +
                capForNullable(firmwareVersion) +
                capForNullable(idfVersion)
                )) {

            writeByte(static_cast<int>(Msg::Type::BRAIN_HELLO));
            writeString(brainId);
            writeNullableString(panelName);
            writeNullableString(firmwareVersion);
            writeNullableString(idfVersion);
        }
    }

    void writeString(const char* sz) {
        if (!sz) return;

        size_t len = strlen(sz);
        size_t xtra = capFor(sz);
        if (prepCapacity(m_used + xtra)) {
            writeInt(len);
            for ( int i = 0; i < len; i++ ) {
                m_buf[m_cursor++] = (uint8_t)sz[i];
            }
            if (m_cursor > m_used) m_used = m_cursor;
        }
    }
*/

pub fn write_hello_msg(w: &mut impl Write, brain_id: &str) {
    /*
            writeByte(BRAIN_HELLO);
            writeString(brainId);
            writeNullableString(panelName);
            writeNullableString(firmwareVersion);
            writeNullableString(idfVersion);
    */
    w.write_all(&[MessageType::BrainHello as u8]).unwrap();
    write_str(w, brain_id);
    write_str_opt(w, None);
    write_str_opt(w, None);
    write_str_opt(w, None);
}

pub fn write_str(w: &mut impl Write, s: &str) {
    let len = s.len() as u32;
    w.write_all(len.to_be_bytes().as_slice()).unwrap();
    w.write_all(s.as_bytes()).unwrap();
}

pub fn write_str_opt(w: &mut impl Write, s: Option<&str>) {
    if let Some(s) = s {
        w.write_all(&[1]).unwrap();
        write_str(w, s);
    } else {
        w.write_all(&[0]).unwrap();
    }
}
