#![feature(alloc)]
extern crate alloc;

use core::cell::RefCell;
use core::fmt;
use core::future::poll_fn;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;
use pretty_hex::*;

use alloc::vec;

use defmt::{info, println};
use embassy_futures::join::join;

use embassy_rp::pac::xip_ctrl::regs::Stat;
use embassy_sync::waitqueue::WakerRegistration;
use embassy_usb::control::{InResponse, OutResponse, Request};
use embassy_usb::descriptor::{SynchronizationType, UsageType};
use embassy_usb::driver::{Driver, Endpoint, EndpointOut};
use embassy_usb::types::InterfaceNumber;
use embassy_usb::types::StringIndex;
use embassy_usb::{Builder, Handler};
use heapless::Vec;

pub struct State<'a> {
    control: MaybeUninit<Control<'a>>,
    shared: ControlShared,
}

impl<'a> Default for State<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> State<'a> {
    /// Create a new `State`.
    pub fn new() -> Self {
        Self {
            control: MaybeUninit::uninit(),
            shared: ControlShared::default(),
        }
    }
}

struct Control<'a> {
    shared: &'a ControlShared,
}

/// Shared data between Control and UAC2
struct ControlShared {
    waker: RefCell<WakerRegistration>,
    changed: AtomicBool,
}

impl Default for ControlShared {
    fn default() -> Self {
        ControlShared {
            waker: RefCell::new(WakerRegistration::new()),
            changed: AtomicBool::new(false),
        }
    }
}

impl ControlShared {
    async fn changed(&self) {
        poll_fn(|cx| {
            if self.changed.load(Ordering::Relaxed) {
                self.changed.store(false, Ordering::Relaxed);
                Poll::Ready(())
            } else {
                self.waker.borrow_mut().register(cx.waker());
                Poll::Pending
            }
        })
        .await;
    }
}

impl<'a> Control<'a> {
    fn shared(&mut self) -> &'a ControlShared {
        self.shared
    }
}

pub struct UAC2<'d, D: Driver<'d>> {
    conf_ep: D::EndpointIn,
    //_data_if: InterfaceNumber,
    read_ep_spk_1: D::EndpointOut,
    read_ep_spk_2: D::EndpointOut,
    write_ep_mic_1: D::EndpointIn,
    write_ep_mic_2: D::EndpointIn,
    //write_ep: D::EndpointIn,
    //control: &'d ControlShared,
    control: &'d ControlShared,
}

impl<'d, D: Driver<'d>> UAC2<'d, D> {
    /// [`Config::composite_with_iads`] must be set, this will add an IAD descriptor.
    /// [`Config::device_class`] = 0xEF
    /// [`Config::device_sub_class`] = 0x02
    /// [`Config::device_protocol`] = 0x01
    pub fn new(builder: &mut Builder<'d, D>, state: &'d mut State<'d>) -> Self {
        let mut fun = builder.function(
            AUDIO_FUNCTION,
            FUNCTION_PROTOCOL_UNDEFINED,
            AF_VERSION_02_00,
        );

        //Standard AC Interface Descriptor(4.7.1)
        let mut int = fun.interface();
        let mut alt_ac = int.alt_setting(AUDIO, AUDIOCONTROL, IP_VERSION_02_00, None);
        let alt_num = alt_ac.alt_setting_number();

        //  Class-Specific AC Interface Header Descriptor(4.7.2)

        //  AudioControl Interface
        let descr_buf_ac_body = vec![
            //  Clock Source Descriptor(4.7.2.1)
            vec![
                8, //Size 8
                CS_INTERFACE,
                CLOCK_SOURCE,
                UAC2_ENTITY_CLOCK, //Clocksource ID
                0b0_11,            //internal programmable clock
                0b01_11,           //frequency RW, validity RO
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
                0b00_00_00_00, //Controls connector none
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
                0b00_00_00_00, //Controls connector none
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
            0x00, //UAC Version BCD (2.0)
            0x02, //
            0x0A, //USB Audio 2.0, PRO-AUDIO
            descr_buf_ac_len[0],
            descr_buf_ac_len[1],
            0,
        ];
        alt_ac.descriptor(CS_INTERFACE, descr_buf_header_ac);
        descr_buf_ac_body
            .iter()
            .for_each(|descriptor| alt_ac.descriptor(descriptor[1], &descriptor[2..]));

        //  Standard AC Interrupt Endpoint Descriptor(4.8.2.1)
        let conf_ep = alt_ac.endpoint_interrupt_in(6, 0x01);

        //Streams for speaker
        //  Standard AS Interface Descriptor(4.9.1)
        let mut int_as_spk = fun.interface();

        //  Interface 1, Alternate 0 - default alternate setting with 0 bandwidth
        let mut alt_as_spk_0 =
            int_as_spk.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);

        //  Interface 1, Alternate 1 - alternate interface for data streaming
        let mut alt_as_spk_1 =
            int_as_spk.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_spk_1 = alt_as_spk_1.alt_setting_number();

        //  Class-Specific AS Interface Descriptor(4.9.2)
        let descr_buf_header_as_spk_1 = &[
            AS_GENERAL,                     //
            UAC2_ENTITY_SPK_INPUT_TERMINAL, //Connected Terminal
            0b00_00,                        // No alternate setting reading
            0x01,                           //AUDIO_FORMAT_TYPE_I (1 byte)
            0x01, //AUDIO_DATA_FORMAT_TYPE_I_PCM (4 bytes) 0x00000001   A.2.1 Audio Data Format Type I Bit Allocations
            0x00, //
            0x00, //
            0x00, //
            2,    //Number of channels
            0x00, //Non predefined channel config 0x00000000
            0x00, //
            0x00, //
            0x00, //
            0x00, //StringIndex Channel name
        ];
        alt_as_spk_1.descriptor(CS_INTERFACE, descr_buf_header_as_spk_1);

        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        let descr_format_spk_1 = &[
            FORMAT_TYPE,
            FORMAT_TYPE_I, //Format Type
            2,             //Subslot Size
            16,            //Resolution
        ];
        alt_as_spk_1.descriptor(CS_INTERFACE, descr_format_spk_1);

        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        let read_ep_spk_1 = alt_as_spk_1.endpoint_isochronous_out(
            196,
            1,
            SynchronizationType::Adaptive,
            UsageType::DataEndpoint,
            &[],
        );

        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        let descr_ep_spk_1 = &[
            EP_GENERAL, //
            0x00,       //Non-max packet size okay
            0b00_00_00, //No Pitch, Data Overrun, Data Underrun
            0x1,        //Lock Delay Unit (Milliseconds)
            0x01,       //Lock Delay (1ms) BE?!
            0x00,       //
        ];
        alt_as_spk_1.descriptor(CS_ENDPOINT, descr_ep_spk_1);

        //  Interface 1, Alternate 2 - alternate interface for data streaming
        let mut alt_as_spk_2 =
            int_as_spk.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_spk_2 = alt_as_spk_2.alt_setting_number();
        //  Class-Specific AS Interface Descriptor(4.9.2)
        let descr_buf_header_as_spk_2 = descr_buf_header_as_spk_1;
        alt_as_spk_2.descriptor(CS_INTERFACE, descr_buf_header_as_spk_2);
        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        let descr_format_spk_1 = &[
            FORMAT_TYPE,
            FORMAT_TYPE_I, //Format Type
            4,             //Subslot Size
            24,            //Resolution
        ];
        alt_as_spk_2.descriptor(CS_INTERFACE, descr_format_spk_1);
        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        let read_ep_spk_2 = alt_as_spk_2.endpoint_isochronous_out(
            392,
            1,
            SynchronizationType::Adaptive,
            UsageType::DataEndpoint,
            &[],
        );
        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        let descr_ep_spk_2 = descr_ep_spk_1;
        alt_as_spk_2.descriptor(CS_ENDPOINT, descr_ep_spk_2);

        //Streams for mic
        //  Standard AS Interface Descriptor(4.9.1)
        let mut int_as_mic = fun.interface();

        //  Interface 1, Alternate 0 - default alternate setting with 0 bandwidth
        let mut alt_as_mic_0 =
            int_as_mic.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);

        //  Interface 1, Alternate 1 - alternate interface for data streaming
        let mut alt_as_mic_1 =
            int_as_mic.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_mic_1 = alt_as_mic_1.alt_setting_number();

        //  Class-Specific AS Interface Descriptor(4.9.2)
        let descr_buf_header_as_mic_1 = &[
            AS_GENERAL,                      //
            UAC2_ENTITY_MIC_OUTPUT_TERMINAL, //Connected Terminal
            0b00_00,                         // No alternate setting reading
            0x01,                            //AUDIO_FORMAT_TYPE_I (1 byte)
            0x01, //AUDIO_DATA_FORMAT_TYPE_I_PCM (4 bytes) 0x00000001   A.2.1 Audio Data Format Type I Bit Allocations
            0x00, //
            0x00, //
            0x00, //
            1,    //Number of channels
            0x00, //Non predefined channel config 0x00000000
            0x00, //
            0x00, //
            0x00, //
            0x00, //StringIndex Channel name
        ];
        alt_as_mic_1.descriptor(CS_INTERFACE, descr_buf_header_as_mic_1);

        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        let descr_format_mic_1 = &[
            FORMAT_TYPE,
            FORMAT_TYPE_I, //Format Type
            2,             //Subslot Size
            16,            //Resolution
        ];
        alt_as_mic_1.descriptor(CS_INTERFACE, descr_format_mic_1);
        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        let write_ep_mic_1 = alt_as_mic_1.endpoint_isochronous_in(
            98,
            1,
            SynchronizationType::Asynchronous,
            UsageType::DataEndpoint,
            &[],
        );

        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        let descr_ep_mic_1 = &[
            EP_GENERAL, //
            0x00,       //Non-max packet size okay
            0b00_00_00, //No Pitch, Data Overrun, Data Underrun
            0x00,       //Lock Delay Unit undefined
            0x00,       //Lock Delay undefined
            0x00,       //
        ];
        alt_as_mic_1.descriptor(CS_ENDPOINT, descr_ep_mic_1);

        //  Interface 1, Alternate 2 - alternate interface for data streaming
        let mut alt_as_mic_2 =
            int_as_mic.alt_setting(AUDIO, AUDIOSTREAMING, IP_VERSION_02_00, None);
        let alt_num_as_mic_2 = alt_as_mic_2.alt_setting_number();

        //  Class-Specific AS Interface Descriptor(4.9.2)
        let descr_buf_header_as_mic_2 = descr_buf_header_as_mic_1;
        alt_as_mic_2.descriptor(CS_INTERFACE, descr_buf_header_as_mic_2);

        //  Type I Format Type Descriptor(2.3.1.6 - Audio Formats)
        let descr_format_mic_2 = &[
            FORMAT_TYPE,
            FORMAT_TYPE_I, //Format Type
            4,             //Subslot Size
            24,            //Resolution
        ];
        alt_as_mic_2.descriptor(CS_INTERFACE, descr_format_mic_2);
        //  Standard AS Isochronous Audio Data Endpoint Descriptor(4.10.1.1
        let write_ep_mic_2 = alt_as_mic_2.endpoint_isochronous_in(
            196,
            1,
            SynchronizationType::Asynchronous,
            UsageType::DataEndpoint,
            &[],
        );
        //  Class-Specific AS Isochronous Audio Data Endpoint Descriptor(4.10.1.2)
        let descr_ep_mic_2 = descr_ep_mic_1;
        alt_as_mic_2.descriptor(CS_ENDPOINT, descr_ep_mic_2);

        let control = state.control.write(Control {
            shared: &state.shared,
        });

        drop(fun);

        builder.handler(control);

        let control_shared = &state.shared;

        UAC2 {
            conf_ep,
            read_ep_spk_1,
            read_ep_spk_2,
            write_ep_mic_1,
            write_ep_mic_2,
            control: control_shared,
        }
    }

    pub fn read_ep_spk_1(&mut self) -> &mut <D as Driver<'d>>::EndpointOut {
        //let mut buf: [u8; 64] = [0; 64];
        &mut self.read_ep_spk_1 //.read(buf.as_mut_slice()).await;
    }

    pub fn read_ep_spk_2(&mut self) -> &mut <D as Driver<'d>>::EndpointOut {
        //let mut buf: [u8; 64] = [0; 64];
        &mut self.read_ep_spk_2 //.read(buf.as_mut_slice()).await;
    }

    pub fn write_ep_mic_1(&mut self) -> &mut <D as Driver<'d>>::EndpointIn {
        //let mut buf: [u8; 64] = [0; 64];
        &mut self.write_ep_mic_1 //.read(buf.as_mut_slice()).await;
    }

    pub fn write_ep_mic_2(&mut self) -> &mut <D as Driver<'d>>::EndpointIn {
        //let mut buf: [u8; 64] = [0; 64];
        &mut self.write_ep_mic_2 //.read(buf.as_mut_slice()).await;
    }

    pub fn conf_ep(self) -> <D as Driver<'d>>::EndpointIn {
        self.conf_ep
    }

    pub async fn stuff(&mut self) {
        let fut_spk_1 = async {
            loop {
                self.read_ep_spk_1.wait_enabled().await;
                info!("Connected");
                loop {
                    let mut data = [0; 196];
                    match self.read_ep_spk_1.read(&mut data).await {
                        Ok(n) => {
                            info!("Got bulk: {:a}", data[..n]);
                            // Echo back to the host:
                            // write_ep.write(&data[..n]).await.ok();
                        }
                        Err(_) => break,
                    }
                }
                info!("Disconnected");
            }
        };
        let fut_spk_2 = async {
            loop {
                self.read_ep_spk_2.wait_enabled().await;
                info!("Connected");
                loop {
                    let mut data = [0; 392];
                    match self.read_ep_spk_2.read(&mut data).await {
                        Ok(n) => {
                            info!("Got bulk: {:a}", data[..n]);
                            // Echo back to the host:
                            // write_ep.write(&data[..n]).await.ok();
                        }
                        Err(_) => break,
                    }
                }
                info!("Disconnected");
            }
        };

        join(fut_spk_1, fut_spk_2).await;
    }
}

impl<'d> Handler for Control<'d> {
    /*
    fn enabled(&mut self, _enabled: bool) {
        info!("enabled");
    }

    fn reset(&mut self) {
        info!("reset");
    }

    */
    fn addressed(&mut self, _addr: u8) {
        info!("addressed {}", _addr);
    }
    /*
    fn configured(&mut self, _configured: bool) {
        info!("configured");
    }

    fn suspended(&mut self, _suspended: bool) {
        info!("suspended");
    }

    fn remote_wakeup_enabled(&mut self, _enabled: bool) {
        info!("remote_wakeup_enabled");
    }
        fn set_alternate_setting(&mut self, iface: InterfaceNumber, alternate_setting: u8) {
            let _ = iface;
            let _ = alternate_setting;
            info!("set_alternate_setting");
        }
    */
    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        let _ = (req, data);
        info!("control_out");
        Some(OutResponse::Accepted)
    }
    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        info!("control_in");
        info!("{:#?}", req);
        info!("{}", buf);

        static vol: &[u8; 8] = &[0x01, 0x00, 0x01, 0x80, 0xFF, 0x7F, 0x01, 0x00];
        static f44_1: [u8; 4] = (44100 as u32).to_le_bytes();
        static f48: [u8; 4] = (48000 as u32).to_le_bytes();
        static freq: &[u8; 26] = &[
            0x02, 0x00, f44_1[0], f44_1[1], f44_1[2], f44_1[3], f44_1[0], f44_1[1], f44_1[2],
            f44_1[3], 0x00, 0x00, 0x00, 0x00, f48[0], f48[1], f48[2], f48[3], f48[0], f48[1],
            f48[2], f48[3], 0x00, 0x00, 0x00, 0x00,
        ];
        if (req.value == 0x0100) {
            Some(InResponse::Accepted(freq))
        } else {
            Some(InResponse::Accepted(vol))
        }
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

const AS_DESCRIPTOR_UNDEFINED: u8 = 0x00;
const AS_GENERAL: u8 = 0x01;
const FORMAT_TYPE: u8 = 0x02;
const ENCODER: u8 = 0x03;

const EP_GENERAL: u8 = 0x01;

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

//FORMAT Type Codes
const FORMAT_TYPE_I: u8 = 0x01;

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
