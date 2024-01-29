#![windows_subsystem = "windows"]

mod app_close_handler;

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::prelude::FileExt,
    sync::{
        atomic::{self, AtomicBool},
        Arc, Mutex,
    },
    thread::{self},
    time::Duration,
};

use anyhow::anyhow;
use chrono::{self, DateTime, TimeZone, Utc};
use flume::{select::SelectError, Selector};
use native_dialog::MessageDialog;

use app_close_handler::AppCloseHandler;

// Define a threshold duration used to determine if the system should initiate a shutdown sequence.
// This constant sets a time limit of 100 seconds. If the duration since the last recorded update 
// (as read from a file) is less than this threshold, it indicates an unexpected restart or a similar
// event. In such a case, the system will consider initiating a shutdown sequence to handle this situation.
static THRESHOLD: Duration = Duration::from_secs(100);

// Specify the timeout duration for the shutdown process. This constant defines a period of 20 seconds
// during which the application will wait after notifying the user of an impending shutdown. If the
// user does not cancel the shutdown within this timeframe, the system will proceed to shut down.
// This timeout provides a brief window for any last-minute user intervention or to abort the shutdown
// process if it was triggered unintentionally.
static SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(20);

fn main() -> anyhow::Result<()> {
    let file = Arc::new(Mutex::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("./last_updated")?,
    ));

    let (shutdown_tx, shutdown_rx) = flume::bounded::<()>(1);
    let (cleanup_tx, cleanup_rx) = flume::bounded::<()>(1);

    //determines weather to shutdown or not.
    //it is not safe to directly call shutdown() inside background worker. all resource(including file) must be released before calling shutdown().
    let shutdown_signal = Arc::new(AtomicBool::new(false));

    //spawn background worker thread that periodically writes current time to file.
    let background_worker = {
        let file = Arc::clone(&file);
        let shutdown_signal = Arc::clone(&shutdown_signal);
        thread::spawn(move || loop {
            let result = Selector::new()
                .recv(&shutdown_rx, |result| {
                    if result.is_ok() {
                        shutdown_signal.store(true, atomic::Ordering::SeqCst)
                    }
                })
                .recv(&cleanup_rx, |_| {})
                .wait_timeout(Duration::from_secs(1));
            match result {
                Ok(_) => {
                    //shutdown or cleanup signal
                    return;
                }
                Err(SelectError::Timeout) => {
                    //timeout expired. update time.
                    write_last_updated(&file.lock().unwrap()).unwrap();
                }
            }
        })
    };

    //if pc is restarted within specified threshold, show shutdown dialog
    if let Ok(duration) = duration_since_shutdown(&file.lock().unwrap()) {
        if duration < THRESHOLD {
            show_shutdown_dialog(SHUTDOWN_TIMEOUT, shutdown_tx);
        }
    }

    let (close_handler_tx, close_handler_rx) = oneshot::channel();

    //install wm_close and wm_endsession handler
    //I can't use ctrlc handler because I'm working on gui mode instead of console mode
    AppCloseHandler::new().on_app_close(move || {
        //send cancel signal to background worker thread
        let _ = cleanup_tx.send(());
        //wait for program exit
        let _ = close_handler_rx.recv();
    });

    //wait for thread to finish
    background_worker.join().unwrap();

    //at this point, file should be flushed and programe is safe to exit.

    //check if shutdown signal is set
    if shutdown_signal.load(atomic::Ordering::SeqCst) {
        //shut down computer
        system_shutdown::shutdown().unwrap();
    }

    //release handler
    let _ = close_handler_tx.send(());
    Ok(())
}

fn show_shutdown_dialog(timeout: Duration, shutdown: flume::Sender<()>) {
    thread::spawn(move || {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        start_shutdown_timeout_thread(timeout, cancel_rx, shutdown);
        MessageDialog::new()
            .set_title("컴퓨터 종료 알림")
            .set_text(&format!(
                "자동 재시작을 감지했습니다. {}초 후 컴퓨터가 종료됩니다.\r\n취소하려면 확인을 누르세요.",
                timeout.as_secs()
            ))
            .show_alert()
            .expect("unable to display dialog box");
        cancel_tx
            .send(())
            .expect("unable to cancel shutdown timeout thread.");
    });
}

fn start_shutdown_timeout_thread(
    timeout: Duration,
    cancel: oneshot::Receiver<()>,
    shutdown: flume::Sender<()>,
) {
    thread::spawn(move || {
        if let Err(oneshot::RecvTimeoutError::Timeout) = cancel.recv_timeout(timeout) {
            //send shutdown signal
            let _ = shutdown.send(());
        }
    });
}

fn duration_since_shutdown(file: &File) -> anyhow::Result<Duration> {
    let now = Utc::now();
    let last_updated = read_last_updated(file)?;
    let duration = (now - last_updated).abs();
    Ok(duration.to_std()?)
}

fn read_last_updated(mut file: &File) -> anyhow::Result<DateTime<Utc>> {
    let mut time = String::new();
    file.read_to_string(&mut time)?;
    let time = time.parse::<i64>()?;
    Utc.timestamp_opt(time, 0)
        .single()
        .ok_or_else(|| anyhow!("Invalid timestamp: {time}"))
}

fn write_last_updated(mut file: &File) -> anyhow::Result<()> {
    let time = Utc::now().timestamp();
    file.seek_write(time.to_string().as_bytes(), 0)?;
    file.flush()?;
    Ok(())
}
