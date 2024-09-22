#![no_std]
#![no_main]

mod uac2;

use core::cell::RefCell;

use cortex_m::register::control::read;
use embassy_futures::join::join;

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_sync::blocking_mutex::NoopMutex;
use embassy_time::{Duration, Timer};
use embassy_usb::driver::{Endpoint, EndpointIn, EndpointOut};
use embassy_usb::msos::{self, windows_version};
use embassy_usb::UsbDevice;
use embedded_alloc::LlffHeap as Heap;
use static_cell::StaticCell;
use uac2::{AudioReaderWriter, State, UAC2};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    use core::mem::MaybeUninit;
    const HEAP_SIZE: usize = 2048;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }

    let p = embassy_rp::init(Default::default());

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs);

    // Create embassy-usb Config
    let config = {
        let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Embassy");
        config.product = Some("UAC2.0 Example");
        config.serial_number = Some("12345678");
        config.max_power = 100;
        config.max_packet_size_0 = 64;

        // Required for windows compatibility.
        // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
        config.device_class = 0xEF;
        config.device_sub_class = 0x02;
        config.device_protocol = 0x01;
        config.composite_with_iads = true;
        config
    };

    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 1024]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
        static MSOS_BUF: StaticCell<[u8; 256]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 1024]),
            BOS_DESCRIPTOR.init([0; 256]),
            MSOS_BUF.init([0; 256]),
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    let mut uac2_class: UAC2<'_, Driver<'_, USB>> = {
        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        UAC2::new(&mut builder, state)
    };
    let mut usb = builder.build();

    let usb_fut = usb.run();

    //let uac2_fut = async { uac2_class.stuff().await };

    let (mut _control, mut reader_writer) = uac2_class.split();

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, stuff(&mut reader_writer)).await;
}

pub async fn stuff<'d>(reader_writer: &mut AudioReaderWriter<'d, Driver<'d, USB>>) {
    struct Sharable {
        mic_data: [u8; 200],
        new_data: bool,
        data_len: usize,
    }
    loop {
        let mut data = [0; 400];
        reader_writer.read_ep_spk.wait_enabled().await;
        info!("Connected");
        match reader_writer.read_ep_spk.read(&mut data).await {
            Ok(n) => {
                info!("Read stuff {:a}", data[..n]);
                //info!("Got bulk: {:a}", data[..n]);
                // Echo back to the host:
                // write_ep.write(&data[..n]).await.ok();

                /*
                let mut sharable = mutex.borrow().borrow_mut();

                data.chunks(4)
                    .zip(sharable.mic_data.chunks_mut(2))
                    .for_each(|(chunk, output)| {
                        let left = u16::from_le_bytes(chunk[0..2].try_into().unwrap());
                        let right = u16::from_le_bytes(chunk[2..4].try_into().unwrap());
                        output.copy_from_slice(
                            &((((left as i16) >> 1) + ((right as i16) >> 1)) as i16).to_le_bytes(),
                        );
                    });
                sharable.data_len = n;
                sharable.new_data = true;
                 */
            }
            Err(error) => {
                info!("Read error {:#?}", error);
                break;
            }
        }
        info!("Disconnected");
        /*             let write_fn = async {
            loop {
                if mutex.borrow().borrow().new_data {
                    let sharable = mutex.borrow().borrow_mut();
                    let _ = reader_writer
                        .write_ep_mic
                        .write(&sharable.mic_data[..sharable.data_len / 2])
                        .await;
                }
            }
        }; */
    }
}
