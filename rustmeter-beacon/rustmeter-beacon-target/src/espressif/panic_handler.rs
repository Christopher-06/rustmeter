#![allow(static_mut_refs)]
use esp_hal::peripherals::{UART0, USB_DEVICE};
use rustmeter_beacon_core::{
    buffer::{ChunkedBufferWriter, SimpleBufferWriter},
    protocol::{CustomPanicInfo, EventPayload},
    time_delta::TimeDelta,
};

use crate::{
    core_id::get_current_core_id,
    espressif::{
        framing::{FrameMode, create_dataframe, create_dataframe_raw},
        tracing_esp::{self, DROPPED_EVENTS_COUNTER},
    },
    timing::get_system_time_us,
};

fn delay_cycles(cycles: u32) {
    for _ in 0..cycles {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    // Take time and block other core / interrupts
    let panic_sys_time = get_system_time_us();
    let mut panic_time_delta = TimeDelta::from_now();
    let _ = unsafe { critical_section::acquire() };

    // Reset UART0 FIFOs
    unsafe {
        let uart0 = &*UART0::PTR;

        // Set reset bit
        uart0
            .conf0()
            .modify(|_, w| w.txfifo_rst().set_bit().rxfifo_rst().set_bit());

        delay_cycles(1000);

        // Clear reset bit
        uart0
            .conf0()
            .modify(|_, w| w.txfifo_rst().clear_bit().rxfifo_rst().clear_bit());
    }

    // USB Serial JTAG Flushing
    unsafe {
        let usb = &*USB_DEVICE::PTR;
        usb.ep1_conf().modify(|_, w| w.wr_done().set_bit());
        delay_cycles(1000);
    }

    // Send rest of tracing data before panic
    let (trace_buffers, _) = tracing_esp::get_tracing_buffers_and_signaller();
    let mut buffer = [0u8; 256]; // Frame buffer (Data Len max 252 bytes + 4 bytes framing)
    let mut seq_id: [usize; _] = [0; 2];
    for (core_id, buf) in trace_buffers.iter().enumerate() {
        let inbuf = unsafe { &mut *buf.get() };

        while inbuf.len() > 0 {
            // peek frame
            let (framelen, datalen) = create_dataframe(
                FrameMode::Panic,
                core_id as u8,
                inbuf,
                &mut buffer,
                seq_id[core_id] as u8,
            );

            // send any data if available
            if datalen > 0 {
                // write frame
                esp_println::Printer::write_bytes(&buffer[0..framelen]);
                inbuf.drain(datalen);

                // Update sequence ID
                seq_id[core_id] = (seq_id[core_id] + 1) % 128;
            }
        }
    }

    // Check for dropped events
    for (core_id, drop_count) in unsafe { DROPPED_EVENTS_COUNTER.iter().enumerate() } {
        if *drop_count > 0 {
            // write dropped events info first
            let drop_event = EventPayload::DataLossEvent {
                dropped_events: *drop_count,
            };

            // Serialize drop event data
            let mut writer = SimpleBufferWriter::new();
            drop_event.write_bytes(&mut writer);
            panic_time_delta.write_bytes(&mut writer);

            // Create frame and send
            let framelen = create_dataframe_raw(
                FrameMode::Panic,
                core_id as u8,
                writer.as_slice(),
                &mut buffer,
                seq_id[core_id] as u8,
            );

            // send any data if available
            if framelen > 0 {
                esp_println::Printer::write_bytes(&buffer[0..framelen]);
                seq_id[core_id] = (seq_id[core_id] + 1) % 128;
            }

            // reset panic time delta count
            if core_id == get_current_core_id() as usize {
                panic_time_delta = TimeDelta::new(0);
            }
        }
    }

    // Create panic event
    let current_core_id = get_current_core_id();
    let msg = EventPayload::Panic(CustomPanicInfo::from_panic(
        info,
        current_core_id,
        panic_sys_time,
    ));

    // Send panic event data
    let mut writer: ChunkedBufferWriter<_, 252> = ChunkedBufferWriter::new(|data| {
        // create frame for panic event and send it immediately
        let core_id = get_current_core_id();
        let frame = create_dataframe_raw(
            FrameMode::Panic,
            core_id,
            data,
            &mut buffer,
            seq_id[core_id as usize] as u8,
        );

        // send frame
        esp_println::Printer::write_bytes(&buffer[0..frame]);
    });
    msg.write_bytes(&mut writer);
    panic_time_delta.write_bytes(&mut writer);
    writer.flush();

    // write raw panic info for optional console attached
    // rustmeter-cli aborts as soon as PanicEvent is received, so this is only for manual debugging
    // strangly this helps with weird issues where sometimes the panic info is not sent correctly but the frames before are fine?!?!?!
    esp_println::println!("\n\r\n\r\n\r!!! PANIC !!!");
    esp_println::println!("{info}\n\r\n\r\n\r");

    // Halt the core
    loop {}
}
