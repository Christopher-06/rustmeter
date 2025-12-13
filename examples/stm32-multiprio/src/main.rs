#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};

use cortex_m_rt::entry;
use defmt::info;
use embassy_executor::{Executor, InterruptExecutor};
use embassy_stm32::interrupt;
use embassy_stm32::interrupt::{InterruptExt, Priority};
use embassy_time::{Duration, Instant, Timer};
use static_cell::StaticCell;

use rustmeter_beacon::*;

// Define three executors for different priority levels
static EXECUTOR_HIGH: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR_MED: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR_LOW: StaticCell<Executor> = StaticCell::new();

#[interrupt]
unsafe fn UART4() {
    unsafe { EXECUTOR_HIGH.on_interrupt() }
}

#[interrupt]
unsafe fn UART5() {
    unsafe { EXECUTOR_MED.on_interrupt() }
}

#[monitor_fn]
fn complex_computation() {
    // Simulate some complex computation
    let start = Instant::now();
    for _ in 0..1234567 {
        cortex_m::asm::nop();
    }

    // Metric reporting of computation duration
    let duration = Instant::now() - start;
    event_metric!("complex_computation_completed", duration.as_millis() as u32);
}

#[entry]
fn main() -> ! {
    let _p = embassy_stm32::init(Default::default());

    // STM32s don’t have any interrupts exclusively for software use, but they can all be triggered by software as well as
    // by the peripheral, so we can just use any free interrupt vectors which aren’t used by the rest of your application.
    // In this case we’re using UART4 and UART5, but there’s nothing special about them. Any otherwise unused interrupt
    // vector would work exactly the same.

    event_metric!("system_startup", 3300);

    // High-priority executor: UART4, priority level 6
    interrupt::UART4.set_priority(Priority::P6);
    let spawner = EXECUTOR_HIGH.start(interrupt::UART4);
    spawner.spawn(hello_world_task_high()).unwrap();
    spawner.spawn(busy_loop_task_high_prio()).unwrap();

    // Medium-priority executor: UART5, priority level 7
    interrupt::UART5.set_priority(Priority::P7);
    let spawner = EXECUTOR_MED.start(interrupt::UART5);
    spawner.spawn(hello_world_task_med()).unwrap();
    spawner.spawn(busy_loop_task_med_prio()).unwrap();

    // Low priority executor: runs in thread mode, using WFE/SEV
    let executor = EXECUTOR_LOW.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(hello_world_task_low()).unwrap();
        spawner.spawn(busy_loop_task_low_prio()).unwrap();
    });
}

#[embassy_executor::task()]
async fn hello_world_task_high() {
    loop {
        // Monitor how long the info! call takes
        monitor_scoped!("hello_world_high", {
            info!("[high] Hello World!");
        });

        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task()]
async fn hello_world_task_med() {
    loop {
        // Monitor how long the info! call takes
        monitor_scoped!("hello_world_med", {
            info!("[med] Hello World!");
        });

        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task()]
async fn hello_world_task_low() {
    loop {
        // Monitor how long the info! call takes
        monitor_scoped!("hello_world_low", {
            info!("[low] Hello World!");
        });

        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Create a task busy looping in a 100ms cycle (95ms sleep, 5ms busy)
#[embassy_executor::task()]
async fn busy_loop_task_high_prio() {
    loop {
        Timer::after(Duration::from_millis(95)).await;

        // Do some complex computation
        complex_computation();
    }
}

/// Create a task busy looping in a 100ms cycle (90ms sleep, 10ms busy)
#[embassy_executor::task()]
async fn busy_loop_task_med_prio() {
    loop {
        Timer::after(Duration::from_millis(90)).await;

        // approx. for 16MHz sysclk (wait ~10ms)
        let cycles = 16_000 * 10;
        for _ in 0..cycles {
            cortex_m::asm::nop();
        }
    }
}

/// Create a task busy looping in a 100ms cycle (80ms sleep, 20ms busy)
#[embassy_executor::task()]
async fn busy_loop_task_low_prio() {
    loop {
        Timer::after(Duration::from_millis(80)).await;

        // approx. for 16MHz sysclk (wait ~20ms)
        let cycles = 16_000 * 20;
        for _ in 0..cycles {
            cortex_m::asm::nop();
        }
    }
}
