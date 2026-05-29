#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::CpuClock,
    interrupt::software::SoftwareInterruptControl,
    peripherals,
    rmt::Rmt,
    time::Rate,
};
// use esp_hal_smartled::{SmartLedsAdapter, smart_led_buffer};
use rustmeter_beacon::*;
// use smart_leds::{
//     SmartLedsWrite as _, brightness,
//     colors::{BLACK, BLUE, GREEN, RED, VIOLET},
// };
use static_cell::StaticCell;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static EXECUTOR_CORE_1: StaticCell<esp_rtos::embassy::Executor> =
    StaticCell::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    // generator version: 1.0.1
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // Initialize Rustmeter Beacon
    init_rustmeter_beacon(
        RustmeterConfig::new(Rate::from_mhz(CpuClock::max() as u32)),
        &spawner
    )
    .unwrap();
    info!("Rustmeter Beacon initialized!");

    info!("Embassy initialized!");
    monitor_value!("system_startup", 3300);

    // Start second core with its own executor
    static APP_CORE_STACK: StaticCell<esp_hal::system::Stack<8192>> = StaticCell::new();
    let app_core_stack = APP_CORE_STACK.init(esp_hal::system::Stack::new());
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL,
        sw_int.software_interrupt1,
        app_core_stack,
        move || {
            let executor = EXECUTOR_CORE_1.init(esp_rtos::embassy::Executor::new());
            executor.run(|spawner| {
                // spawner
                //     .spawn(blink_led_task(peripherals.GPIO48, peripherals.RMT).unwrap());
                spawner.spawn(hello_world_task().unwrap());
                spawner.spawn(busy_loop_task_second().unwrap());
            });
        },
    );
    info!("Second Core Executor started!");

    // Spawn tasks on core 0
    spawner.spawn(busy_loop_task().unwrap());

    loop {
        // main task does nothing
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[monitor_fn]
fn busy_loop_simulation(ms: u64) {
    let start = embassy_time::Instant::now();
    while (embassy_time::Instant::now() - start).as_millis() < ms {
        // do nothing
    }
}

#[monitor_fn]
async fn complex_calc() {
    // Simulate some complex computation
    let start = embassy_time::Instant::now();
    step!("busy looping 15ms");
    busy_loop_simulation(15);
    step!("busy looping 20ms");
    busy_loop_simulation(10);
    busy_loop_simulation(10);

    Timer::after_millis(25).await;
    step!("after 25ms timer");

    let time_took = ((embassy_time::Instant::now() - start).as_micros() % 10) as u32;
    monitor_value!("complex_comp_done", time_took);
}

/// Create a task that prints "Hello World" every second
#[embassy_executor::task]
async fn hello_world_task() {
    loop {
        info!("Hello, world!");
        Timer::after(Duration::from_secs(1)).await;
        complex_calc().await;
    }
}

/// Create a task that blinks an LED every 500ms
// TODO: Currently waits for esp-hal-smartled crate to accept esp-hal 1.1.1, then re-add smart-leds to Cargo.toml and uncomment this task
// #[embassy_executor::task]
// async fn blink_led_task(led: peripherals::GPIO48<'static>, rmt: peripherals::RMT<'static>) {
//     let rmt = Rmt::new(rmt, Rate::from_mhz(80)).expect("Failed to initialize RMT");

//     let rmt_channel = rmt.channel0;
//     let mut rmt_buffer = smart_led_buffer!(1);

//     let mut led = SmartLedsAdapter::new(rmt_channel, led, &mut rmt_buffer);

//     for _ in 0..3 {
//         led.write(brightness([VIOLET].into_iter(), 10)).unwrap();
//         Timer::after(Duration::from_millis(500)).await;
//         led.write(brightness([BLACK].into_iter(), 10)).unwrap();
//         Timer::after(Duration::from_millis(500)).await;
//     }

//     let mut pixel = RED;
//     loop {
//         led.write(brightness([pixel].into_iter(), 10)).unwrap();

//         // Swithc between RED, Green, BLUE
//         pixel = match pixel {
//             RED => GREEN,
//             GREEN => BLUE,
//             BLUE => RED,
//             _ => RED,
//         };

//         Timer::after(Duration::from_millis(300)).await;
//     }
// }

/// Create a second task busy looping in a 10ms cycle
#[embassy_executor::task]
async fn busy_loop_task_second() {
    loop {
        Timer::after(Duration::from_micros(5000)).await;

        let start = embassy_time::Instant::now();
        while (embassy_time::Instant::now() - start).as_micros() < 5000 {
            // do nothing
        }
    }
}

/// Create a task busy looping in a 10ms cycle
#[embassy_executor::task()]
async fn busy_loop_task() {
    loop {
        Timer::after(Duration::from_millis(5)).await;

        // wait for 5ms while busy looping, simulating a blocking operation
        let start = embassy_time::Instant::now();
        let x = monitor_scoped!("BusyLoopScoped", {
            while (embassy_time::Instant::now() - start).as_millis() < 5 {
                // do nothing
            }

            5
        });
        assert!(x == 5);
    }
}
