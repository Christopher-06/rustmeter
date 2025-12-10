#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::*;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use rustmeter_beacon::*;
use static_cell::StaticCell;
use {esp_backtrace as _, esp_println as _};

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static EXECUTOR_CORE_1: static_cell::StaticCell<esp_rtos::embassy::Executor> =
    static_cell::StaticCell::new();

#[monitor_fn(name = "busy_loop_simulation")]
fn busy_loop_simulation(ms: u64) {
    let start = embassy_time::Instant::now();
    while (embassy_time::Instant::now() - start).as_millis() < ms {
        // do nothing
    }
}

#[monitor_fn]
fn complex_computation() {
    // Simulate some complex computation
    let start = embassy_time::Instant::now();
    busy_loop_simulation(15);
    busy_loop_simulation(10);
    busy_loop_simulation(5);

    let time_took = ((embassy_time::Instant::now() - start).as_micros() % 10) as u32;
    event_metric!("complex_computation_completed", time_took);
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    // generator version: 1.0.1

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    let led: esp_hal::gpio::Output<'_> = gpio::Output::new(
        peripherals.GPIO2,
        gpio::Level::High,
        gpio::OutputConfig::default(),
    );

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");

    event_metric!("system_startup", 3300);

    // Start second core with its own executor
    static APP_CORE_STACK: StaticCell<esp_hal::system::Stack<8192>> = StaticCell::new();
    let app_core_stack = APP_CORE_STACK.init(esp_hal::system::Stack::new());
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL,
        sw_int.software_interrupt0,
        sw_int.software_interrupt1,
        app_core_stack,
        move || {
            let executor = EXECUTOR_CORE_1.init(esp_rtos::embassy::Executor::new());
            executor.run(|spawner| {
                spawner.spawn(busy_loop_task_second()).unwrap();
            });
        },
    );
    info!("Second Core Interrupt Executor started!");

    // Spawn tasks on core 0
    spawner.spawn(hello_world_task()).unwrap();
    spawner.spawn(blink_led_task(led)).unwrap();
    spawner.spawn(busy_loop_task()).unwrap();

    loop {
        // main task does nothing
        Timer::after(Duration::from_secs(60)).await;
    }
}

/// Create a task that prints "Hello World" every second
#[embassy_executor::task]
async fn hello_world_task() {
    loop {
        info!("Hello, world!");
        Timer::after(Duration::from_secs(1)).await;
        complex_computation();
    }
}

/// Create a task that blinks an LED every 500ms
#[embassy_executor::task]
async fn blink_led_task(mut led: esp_hal::gpio::Output<'static>) {
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}

/// Create a task busy looping in a 100ms cycle
#[embassy_executor::task()]
async fn busy_loop_task() {
    loop {
        Timer::after(Duration::from_millis(70)).await;

        monitor_scoped!("BusyLoopComputation via Scoped Monitor", {
            let start = embassy_time::Instant::now();
            while (embassy_time::Instant::now() - start).as_millis() < 30 {
                // do nothing
            }
        });
    }
}

/// Create a second task busy looping in a 1000ms cycle
#[embassy_executor::task]
async fn busy_loop_task_second() {
    loop {
        Timer::after(Duration::from_millis(30)).await;

        let start = embassy_time::Instant::now();
        while (embassy_time::Instant::now() - start).as_millis() < 70 {
            // do nothing
        }
    }
}
