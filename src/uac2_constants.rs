#![allow(dead_code)]

// UAC2 standard
pub const AUDIO: u8 = 0x01;
pub const AUDIOCONTROL: u8 = 0x01;
pub const AUDIOSTREAMING: u8 = 0x02;
pub const IP_VERSION_02_00: u8 = 0x20;

pub const AUDIO_FUNCTION: u8 = AUDIO;
pub const FUNCTION_PROTOCOL_UNDEFINED: u8 = 0x00;
pub const AF_VERSION_02_00: u8 = IP_VERSION_02_00;

pub const CS_STRING: u8 = 0x23;
pub const CS_INTERFACE: u8 = 0x24;
pub const CS_ENDPOINT: u8 = 0x25;

pub const AS_DESCRIPTOR_UNDEFINED: u8 = 0x00;
pub const AS_GENERAL: u8 = 0x01;
pub const FORMAT_TYPE: u8 = 0x02;
pub const ENCODER: u8 = 0x03;

pub const EP_GENERAL: u8 = 0x01;

pub const AC_DESCRIPTOR_UNDEFINED: u8 = 0x00;
pub const HEADER: u8 = 0x01;
pub const INPUT_TERMINAL: u8 = 0x02;
pub const OUTPUT_TERMINAL: u8 = 0x03;
pub const MIXER_UNIT: u8 = 0x04;
pub const SELECTOR_UNIT: u8 = 0x05;
pub const FEATURE_UNIT: u8 = 0x06;
pub const EFFECT_UNIT: u8 = 0x07;
pub const PROCESSING_UNIT: u8 = 0x08;
pub const EXTENSION_UNIT: u8 = 0x09;
pub const CLOCK_SOURCE: u8 = 0x0A;
pub const CLOCK_SELECTOR: u8 = 0x0B;
pub const CLOCK_MULTIPLIER: u8 = 0x0C;
pub const SAMPLE_RATE_CONVERTER: u8 = 0x0D;

//Requests
pub const REQUEST_CODE_UNDEFINED: u8 = 0x00;
pub const CUR: u8 = 0x01;
pub const RANGE: u8 = 0x02;

pub const FU_CONTROL_UNDEFINED: u8 = 0x00;
pub const FU_MUTE_CONTROL: u8 = 0x01;
pub const FU_VOLUME_CONTROL: u8 = 0x02;

//USB Terminal Types
pub const USB_UNDEFINED: [u8; 2] = (0x0100 as u16).to_le_bytes();
pub const USB_STREAM: [u8; 2] = (0x0101 as u16).to_le_bytes();
pub const USB_VENDOR: [u8; 2] = (0x01FF as u16).to_le_bytes();

//Input Terminal Types
pub const INPUT_UNDEFINED: [u8; 2] = (0x0200 as u16).to_le_bytes();
pub const INPUT_MICROPHONE: [u8; 2] = (0x0201 as u16).to_le_bytes();
pub const INPUT_DESKTOP_MICROPHONE: [u8; 2] = (0x0202 as u16).to_le_bytes();
pub const INPUT_PERSONAL_MICROPHONE: [u8; 2] = (0x0203 as u16).to_le_bytes();
pub const INPUT_OMNIDIRECTIONAL_MICROPHONE: [u8; 2] = (0x0204 as u16).to_le_bytes();
pub const INPUT_MICROPHONE_ARRAY: [u8; 2] = (0x0205 as u16).to_le_bytes();
pub const INPUT_PROCESSING_MICROPHONE_ARRAY: [u8; 2] = (0x0206 as u16).to_le_bytes();

//OUTPUT Terminal Types
pub const OUTPUT_UNDEFINED: [u8; 2] = (0x0300 as u16).to_le_bytes();
pub const OUTPUT_SPEAKER: [u8; 2] = (0x0301 as u16).to_le_bytes();
pub const OUTPUT_HEADPHONES: [u8; 2] = (0x0302 as u16).to_le_bytes();
pub const OUTPUT_HMD_AUDIO: [u8; 2] = (0x0303 as u16).to_le_bytes();
pub const OUTPUT_DESKTOP_SPEAKER: [u8; 2] = (0x0304 as u16).to_le_bytes();
pub const OUTPUT_ROOM_SPEAKER: [u8; 2] = (0x0305 as u16).to_le_bytes();
pub const OUTPUT_COMMUNICATION_SPEAKER: [u8; 2] = (0x0306 as u16).to_le_bytes();
pub const OUTPUT_LFE_SPEAKER: [u8; 2] = (0x0307 as u16).to_le_bytes();

//FORMAT Type Codes
pub const FORMAT_TYPE_I: u8 = 0x01;

//Demo pub constants

// Unit numbers are arbitrary selected
pub const UAC2_ENTITY_CLOCK: u8 = 0x04;
// Speaker path
pub const UAC2_ENTITY_SPK_INPUT_TERMINAL: u8 = 0x01;
pub const UAC2_ENTITY_SPK_FEATURE_UNIT: u8 = 0x02;
pub const UAC2_ENTITY_SPK_OUTPUT_TERMINAL: u8 = 0x03;
// Microphone path
pub const UAC2_ENTITY_MIC_INPUT_TERMINAL: u8 = 0x11;
pub const UAC2_ENTITY_MIC_OUTPUT_TERMINAL: u8 = 0x13;
