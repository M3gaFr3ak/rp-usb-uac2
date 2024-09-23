#![no_std]
#![no_main]

mod uac2;
mod uac2_constants;

use core::f32::consts::E;
use core::sync::atomic::AtomicBool;

use embassy_futures::join::join;

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, ThreadModeRawMutex};
use embassy_sync::signal::Signal;
use embassy_time::{Instant, Timer};
use embassy_usb::UsbDevice;
use embedded_alloc::LlffHeap as Heap;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use static_cell::StaticCell;
use uac2::{AudioReader, AudioReaderWriter, AudioWriter, ControlChanged, State, UAC2};
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

    let uac2_class: UAC2<'_, Driver<'_, USB>> = {
        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        UAC2::new(&mut builder, state)
    };
    let usb = builder.build();

    unwrap!(spawner.spawn(usb_task(usb)));
    //let uac2_fut = async { uac2_class.stuff().await };

    let (mut _control, reader_writer): (
        ControlChanged<'_>,
        AudioReaderWriter<'_, Driver<'_, USB>>,
    ) = uac2_class.split();

    let (mut reader, mut writer) = reader_writer.split();

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(receive_task(&mut reader), send_task(&mut writer)).await;
}

type MyUsbDriver = Driver<'static, USB>;
type MyUsbDevice = UsbDevice<'static, MyUsbDriver>;

#[embassy_executor::task]
async fn usb_task(mut usb: MyUsbDevice) -> ! {
    usb.run().await
}

pub async fn send_task<'d, T: Instance + 'd>(writer: &mut AudioWriter<'d, Driver<'d, T>>) {
    let mut data: [u8; 98] = [0; 98];
    let mut small_rng = SmallRng::seed_from_u64(0x3675978356739456);
    data.iter_mut()
        .enumerate()
        .for_each(|a| *a.1 = small_rng.gen());

    loop {
        writer.wait_enabled_16().await;
        info!("Connected");

        let mut last_micros: u64 = 0;
        loop {
            let first: embassy_futures::select::Either<(), AtomicBool> =
                select(Timer::after_micros(900), SIGNAL.wait()).await;
            let mut write_fut: Result<(), embassy_usb::driver::EndpointError>;
            if let Either::Second(_) = first {
                SIGNAL.reset();
                write_fut = writer.write_16(unsafe { &BUFFER }).await;
            }else {
                write_fut = writer.write_16(&data).await
            }
            //let mut data_new = Option::None;
            //while (data_new.is_none()) {
            //    data_new = SIGNAL.try_take();
            //}

            //match writer.write_16(&data).await {
            match write_fut {
                Ok(_) => {
                    //info!("Sent stuff");
                    //let current_micros = Instant::now().as_micros();
                    //let delta = current_micros - last_micros;
                    //last_micros = current_micros;
                    //info!("Send delta: {}", delta);
                }
                Err(error) => {
                    info!("Write error {:#?}", error);
                    break;
                }
            }
        }
        info!("Disconnected");
    }
}

static SIGNAL: Signal<ThreadModeRawMutex, AtomicBool> = Signal::new();
static mut BUFFER: [u8; 98] = [0; 98];

pub async fn receive_task<'d, T: Instance + 'd>(reader: &mut AudioReader<'d, Driver<'d, T>>) {
    loop {
        let mut data = [0; 400];
        reader.wait_enabled_16().await;
        info!("Connected");

        let mut last_micros: u64 = 0;
        loop {
            match reader.read_16(&mut data).await {
                Ok(n) => {
                    //info!("Read stuff {} bytes", n);
                    //let current_micros = Instant::now().as_micros();
                    //let delta = current_micros - last_micros;
                    //last_micros = current_micros;

                    //info!("Read delta: {}", delta);
                    //info!("Got bulk: {:a}", data[..n]);
                    // Echo back to the host:
                    // write_ep.write(&data[..n]).await.ok();

                    let mut _mic_data: [u8; 98] = [0; 98];
                    data.chunks(4)
                        .zip(_mic_data.chunks_mut(2))
                        .for_each(|(chunk, output)| {
                            let left = i16::from_le_bytes(chunk[0..2].try_into().unwrap());
                            let right = i16::from_le_bytes(chunk[2..4].try_into().unwrap());
                            output.copy_from_slice(
                                &((((left as i16) >> 1) + ((right as i16) >> 1)) as i16)
                                    .to_le_bytes(),
                            );
                        });
                    let _data_len: usize = n / 2;

                    unsafe { BUFFER.copy_from_slice(&_mic_data) };
                    SIGNAL.signal(AtomicBool::new(true));
                    //let chunking_micros = Instant::now().as_micros();
                    //info!("Chunking time: {}", chunking_micros - current_micros);
                }
                Err(error) => {
                    info!("Read error {:#?}", error);
                    break;
                }
            }
        }
        info!("Disconnected");
    }
}
