#![feature(alloc)]
extern crate alloc;

use alloc::{vec, vec::Vec};

use embassy_usb::descriptor::descriptor_type::INTERFACE;
use embassy_usb::driver::{Driver, EndpointOut};
use embassy_usb::types::InterfaceNumber;
use embassy_usb::Builder;

pub struct UAC2<'d, D: Driver<'d>> {
    conf_ep: D::EndpointIn,
    //_data_if: InterfaceNumber,
    read_ep: D::EndpointOut,
    //write_ep: D::EndpointIn,
    //control: &'d ControlShared,
}

impl<'d, D: Driver<'d>> UAC2<'d, D> {
    /// [`Config::composite_with_iads`] must be set, this will add an IAD descriptor.
    /// [`Config::device_class`] = 0xEF
    /// [`Config::device_sub_class`] = 0x02
    /// [`Config::device_protocol`] = 0x01
    pub fn new(builder: &mut Builder<'d, D>, max_packet_size: u16) -> Self {
        let mut fun = builder.function(
            AUDIO_FUNCTION,
            FUNCTION_PROTOCOL_UNDEFINED,
            AF_VERSION_02_00,
        );

        //Standard AC Interface Descriptor(4.7.1)
        let mut int = fun.interface();

        //  Class-Specific AC Interface Header Descriptor(4.7.2)
        let mut alt_ac = int.alt_setting(AUDIO, AUDIOCONTROL, IP_VERSION_02_00, None);
        let alt_num = alt_ac.alt_setting_number();

        //  AudioControl Interface
        let descr_buf_ac_body = vec![
            //  Clock Source Descriptor(4.7.2.1)
            vec![
                8, //Size 8
                CS_INTERFACE,
                CLOCK_SOURCE,
                UAC2_ENTITY_CLOCK, //Clocksource ID
                0b0_10,
                0b11_11,
                0,
                0,
            ],
            //  Input Terminal Descriptor(4.7.2.4)
            vec![
                17,
                CS_INTERFACE,
                INPUT_TERMINAL,
                UAC2_ENTITY_SPK_INPUT_TERMINAL, //Terminal ID
                USB_STREAM[0],                  //Terminal Type
                USB_STREAM[1],
                0x00,              //No associated terminal
                UAC2_ENTITY_CLOCK, //Clocksource ID
                2,                 //2 logical audio channels
                0x00,              //Channel config
                0x00,
                0x00,
                0x00,
                0x00,          //Channel names string index
                0b00_00_01_00, //Controls connector status readable
                0b00_00,
                0x00, //Terminal description string index
            ],
            //  Feature Unit Descriptor(4.7.2.8)
            vec![
                6 + (2 + 1) * 4, //Length 6+(ch+1)*4 for ch=2
                CS_INTERFACE,
                FEATURE_UNIT,
                UAC2_ENTITY_SPK_FEATURE_UNIT,   //Unit ID
                UAC2_ENTITY_SPK_INPUT_TERMINAL, //Source ID
                0b00_00_11_11,                  //Control Masterchannel
                0x00,
                0x00,
                0x00,
                0b00_00_11_11, //Control Channel 1
                0x00,
                0x00,
                0x00,
                0b00_00_11_11, //Control Channel 2
                0x00,
                0x00,
                0x00,
                0x00, //No String Descriptor
            ],
            //  Output Terminal Descriptor(4.7.2.5)
            vec![
                12, //Size 12
                CS_INTERFACE,
                OUTPUT_TERMINAL,
                UAC2_ENTITY_SPK_OUTPUT_TERMINAL, // Terminal ID
                OUTPUT_SPEAKER[0],               //Terminal Type
                OUTPUT_SPEAKER[1],
                0x00,                         //No associated terminal
                UAC2_ENTITY_SPK_FEATURE_UNIT, //Source ID
                UAC2_ENTITY_CLOCK,            //Clocksource ID
                0x00,                         //No controls
                0x00,
                0x00, //No String Descriptor
            ],
            //  Input Terminal Descriptor(4.7.2.4)
            vec![
                17,
                CS_INTERFACE,
                INPUT_TERMINAL,
                UAC2_ENTITY_MIC_INPUT_TERMINAL, //Terminal ID
                INPUT_MICROPHONE[0],            //Terminal Type
                INPUT_MICROPHONE[1],
                0x00,              //No associated terminal
                UAC2_ENTITY_CLOCK, //Clocksource ID
                1,                 //1 logical audio channel
                0x00,              //Channel config
                0x00,
                0x00,
                0x00,
                0x00,          //Channel names string index
                0b00_00_01_00, //Controls connector status readable
                0b00_00,
                0x00, //Terminal description string index
            ],
            //  Output Terminal Descriptor(4.7.2.5)
            vec![
                12, //Size 12
                CS_INTERFACE,
                OUTPUT_TERMINAL,
                UAC2_ENTITY_MIC_OUTPUT_TERMINAL, // Terminal ID
                USB_STREAM[0],                   //Terminal Type
                USB_STREAM[1],
                0x00,                           //No associated terminal
                UAC2_ENTITY_MIC_INPUT_TERMINAL, //Source ID
                UAC2_ENTITY_CLOCK,              //Clocksource ID
                0x00,                           //No controls
                0x00,
                0x00, //No string
            ],
        ];

        let descr_buf_ac_body_len = descr_buf_ac_body.iter().map(|vec| vec.len()).sum::<usize>();
        let descr_buf_ac_len = (descr_buf_ac_body_len + 9).to_le_bytes(); //Class-Specific AC Interface Header Descriptor length = 9, wTotalLength = sum of length of all CS AC IF descriptors including header descriptor

        //  Class-Specific AC Interface Header Descriptor(4.7.2)
        let descr_buf_header_ac = &[
            HEADER,
            0x02,
            0x00,
            0x0A, //USB Audio 2.0, PRO-AUDIO
            descr_buf_ac_len[0],
            descr_buf_ac_len[1],
            0,
        ];
        alt_ac.descriptor(CS_INTERFACE, descr_buf_header_ac);
        descr_buf_ac_body
            .iter()
            .for_each(|descriptor| alt_ac.descriptor(descriptor[1], &descriptor[2..]));
        alt_ac.descriptor(CS_INTERFACE, descr_buf_header_ac);

        //  Standard AC Interrupt Endpoint Descriptor(4.8.2.1)
        let conf_ep = alt_ac.endpoint_interrupt_in(6, 0x01);

        //Streams for speaker
        //  Standard AS Interface Descriptor(4.9.1)
        let mut int_as = fun.interface();

        //  Interface 1, Alternate 0 - default alternate setting with 0 bandwidth
        let mut alt_as_0 = int_as.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);

        //  Interface 1, Alternate 1 - alternate interface for data streaming
        let mut alt_as_1 = int_as.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_1 = alt_as_1.alt_setting_number();

        //  Class-Specific AS Interface Descriptor(4.9.2)
        let descr_buf_header_as = &[
            AS_GENERAL, 0x2, //Connected Terminal
            0b11_11, 0x01, //AUDIO_FORMAT_TYPE_I (1 byte)
            0x00, //AUDIO_DATA_FORMAT_TYPE_I_PCM (4 bytes) 0x00000001
            0x00, 0x00, 0x01, 0x01, //Number of channels
            0x00, //Non predefined channel config 0x00000000
            0x00, 0x00, 0x00, 0x00, //StringIndex Channel name
        ];
        alt_as_1.descriptor(CS_INTERFACE, descr_buf_header_as);

        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        todo!();

        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        let read_ep = alt_as_1.endpoint_isochronous_out(64, 1);

        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        todo!();

        //  Interface 1, Alternate 2 - alternate interface for data streaming
        let mut alt_as_2 = int_as.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_2 = alt_as_2.alt_setting_number();
        //  Class-Specific AS Interface Descriptor(4.9.2)
        todo!();
        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        todo!();
        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        todo!();
        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        todo!();

        //Streams for mic
        //  Standard AS Interface Descriptor(4.9.1)
        let mut int_as = fun.interface();

        //  Interface 1, Alternate 0 - default alternate setting with 0 bandwidth
        let mut alt_as_0 = int_as.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);

        //  Interface 1, Alternate 1 - alternate interface for data streaming
        let mut alt_as_1 = int_as.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_1 = alt_as_1.alt_setting_number();

        //  Class-Specific AS Interface Descriptor(4.9.2)
        let descr_buf_header_as = &[
            AS_GENERAL, 0x2, //Connected Terminal
            0b11_11, 0x01, //AUDIO_FORMAT_TYPE_I (1 byte)
            0x00, //AUDIO_DATA_FORMAT_TYPE_I_PCM (4 bytes) 0x00000001
            0x00, 0x00, 0x01, 0x01, //Number of channels
            0x00, //Non predefined channel config 0x00000000
            0x00, 0x00, 0x00, 0x00, //StringIndex Channel name
        ];
        alt_as_1.descriptor(CS_INTERFACE, descr_buf_header_as);

        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        todo!();

        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        let write_ep = alt_as_1.endpoint_isochronous_in(64, 1);

        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        todo!();

        //  Interface 1, Alternate 2 - alternate interface for data streaming
        let mut alt_as_2 = int_as.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_2 = alt_as_2.alt_setting_number();
        //  Class-Specific AS Interface Descriptor(4.9.2)
        todo!();
        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        todo!();
        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        todo!();
        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        todo!();

        UAC2 {
            conf_ep,
            read_ep,
            //write_ep,
        }
    }

    pub fn read_ep(&mut self) -> &mut <D as Driver<'d>>::EndpointOut {
        //let mut buf: [u8; 64] = [0; 64];
        &mut self.read_ep //.read(buf.as_mut_slice()).await;
    }

    //pub fn write_ep(self) -> <D as Driver<'d>>::EndpointIn {
    //    self.write_ep
    //}

    pub fn conf_ep(self) -> <D as Driver<'d>>::EndpointIn {
        self.conf_ep
    }
}

// UAC2 standard
const AUDIO: u8 = 0x01;
const AUDIOCONTROL: u8 = 0x01;
const AUDIOSTREAMING: u8 = 0x02;
const IP_VERSION_02_00: u8 = 0x20;

const AUDIO_FUNCTION: u8 = AUDIO;
const FUNCTION_PROTOCOL_UNDEFINED: u8 = 0x00;
const AF_VERSION_02_00: u8 = IP_VERSION_02_00;

const CS_STRING: u8 = 0x23;
const CS_INTERFACE: u8 = 0x24;
const CS_ENDPOINT: u8 = 0x25;

const AS_GENERAL: u8 = 0x01;

const AC_DESCRIPTOR_UNDEFINED: u8 = 0x00;
const HEADER: u8 = 0x01;
const INPUT_TERMINAL: u8 = 0x02;
const OUTPUT_TERMINAL: u8 = 0x03;
const MIXER_UNIT: u8 = 0x04;
const SELECTOR_UNIT: u8 = 0x05;
const FEATURE_UNIT: u8 = 0x06;
const EFFECT_UNIT: u8 = 0x07;
const PROCESSING_UNIT: u8 = 0x08;
const EXTENSION_UNIT: u8 = 0x09;
const CLOCK_SOURCE: u8 = 0x0A;
const CLOCK_SELECTOR: u8 = 0x0B;
const CLOCK_MULTIPLIER: u8 = 0x0C;
const SAMPLE_RATE_CONVERTER: u8 = 0x0D;

//USB Terminal Types
const USB_UNDEFINED: [u8; 2] = (0x0100 as u16).to_le_bytes();
const USB_STREAM: [u8; 2] = (0x0101 as u16).to_le_bytes();
const USB_VENDOR: [u8; 2] = (0x01FF as u16).to_le_bytes();

//Input Terminal Types
const INPUT_UNDEFINED: [u8; 2] = (0x0200 as u16).to_le_bytes();
const INPUT_MICROPHONE: [u8; 2] = (0x0201 as u16).to_le_bytes();
const INPUT_DESKTOP_MICROPHONE: [u8; 2] = (0x0202 as u16).to_le_bytes();
const INPUT_PERSONAL_MICROPHONE: [u8; 2] = (0x0203 as u16).to_le_bytes();
const INPUT_OMNIDIRECTIONAL_MICROPHONE: [u8; 2] = (0x0204 as u16).to_le_bytes();
const INPUT_MICROPHONE_ARRAY: [u8; 2] = (0x0205 as u16).to_le_bytes();
const INPUT_PROCESSING_MICROPHONE_ARRAY: [u8; 2] = (0x0206 as u16).to_le_bytes();

//OUTPUT Terminal Types
const OUTPUT_UNDEFINED: [u8; 2] = (0x0300 as u16).to_le_bytes();
const OUTPUT_SPEAKER: [u8; 2] = (0x0301 as u16).to_le_bytes();
const OUTPUT_HEADPHONES: [u8; 2] = (0x0302 as u16).to_le_bytes();
const OUTPUT_HMD_AUDIO: [u8; 2] = (0x0303 as u16).to_le_bytes();
const OUTPUT_DESKTOP_SPEAKER: [u8; 2] = (0x0304 as u16).to_le_bytes();
const OUTPUT_ROOM_SPEAKER: [u8; 2] = (0x0305 as u16).to_le_bytes();
const OUTPUT_COMMUNICATION_SPEAKER: [u8; 2] = (0x0306 as u16).to_le_bytes();
const OUTPUT_LFE_SPEAKER: [u8; 2] = (0x0307 as u16).to_le_bytes();

//Output Terminal Types

//Demo constants

// Unit numbers are arbitrary selected
const UAC2_ENTITY_CLOCK: u8 = 0x04;
// Speaker path
const UAC2_ENTITY_SPK_INPUT_TERMINAL: u8 = 0x01;
const UAC2_ENTITY_SPK_FEATURE_UNIT: u8 = 0x02;
const UAC2_ENTITY_SPK_OUTPUT_TERMINAL: u8 = 0x03;
// Microphone path
const UAC2_ENTITY_MIC_INPUT_TERMINAL: u8 = 0x11;
const UAC2_ENTITY_MIC_OUTPUT_TERMINAL: u8 = 0x13;
