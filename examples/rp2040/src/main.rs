//! An example demonstrating the use of Rustmeter Beacon with Embassy on a Raspberry Pi Pico (RP2040).
//! This example sets up multiple asynchronous tasks with different priorities across both cores of the RP2040,
//! including tasks that perform long computations and control an LED (everything monitored)
//! 
#![no_std]
#![no_main]

use cortex_m::asm;
use defmt::*;
use embassy_executor::{Executor, InterruptExecutor};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::{
    interrupt,
    interrupt::{InterruptExt, Priority},
    multicore::{Stack, spawn_core1},
};
use static_cell::StaticCell;
use embassy_time::Timer;
use panic_probe as _;
use rustmeter_beacon::{monitor_fn, monitor_scoped, rustmeter_init_default};

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

static EXECUTOR0_LOW: StaticCell<Executor> = StaticCell::new();
static EXECUTOR0_HIGH: InterruptExecutor = InterruptExecutor::new();

#[interrupt]
unsafe fn SWI_IRQ_0() {
    unsafe { EXECUTOR0_HIGH.on_interrupt() }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    rustmeter_init_default();

    let p = embassy_rp::init(Default::default());
    let led = Output::new(p.PIN_25, Level::Low);

    // Spawn core 1
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                spawner.spawn(hello_world_task_core1().unwrap());
            });
        },
    );

    // Start executor on core 0 (high priority)
    interrupt::SWI_IRQ_0.set_priority(Priority::P3);
    let spawner = EXECUTOR0_HIGH.start(interrupt::SWI_IRQ_0);
    spawner.spawn(spamming_task().unwrap());

    // Start executor on core 0 (low priority)
    let executor0 = EXECUTOR0_LOW.init(Executor::new());
    executor0.run(|spawner| {
        spawner.spawn(led_blinky_task(led).unwrap());
        spawner.spawn(long_computation_task().unwrap());
        spawner.spawn(hello_world_task_core0().unwrap());
    });
}

#[embassy_executor::task]
async fn spamming_task() {
    loop {
        let start = embassy_time::Instant::now();
        while embassy_time::Instant::now() - start < embassy_time::Duration::from_micros(1500) {}

        Timer::after(embassy_time::Duration::from_micros(1500)).await;
    }
}

#[embassy_executor::task]
async fn long_computation_task() {
    loop {
        do_long_computation();

        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}

#[monitor_fn]
#[inline(never)]
fn do_long_computation() {
    for _ in 0..1_000_000 {
        asm::nop();
    }
}

#[embassy_executor::task]
async fn hello_world_task_core0() {
    loop {
        info!("[core0] Hello, world!");

        Timer::after_secs(1).await;

        monitor_scoped!("noop", {
            asm::nop();
        });
    }
}

#[embassy_executor::task]
async fn hello_world_task_core1() {
    loop {
        monitor_scoped!("hello_core1", {
            info!("[core1] Hello, world!");
        });

        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn led_blinky_task(mut led: Output<'static>) {
    loop {
        monitor_scoped!("led_set_high", {
            led.set_high();
        });
        Timer::after_secs(1).await;

        led.set_low();
        Timer::after_secs(1).await;
    }
}
