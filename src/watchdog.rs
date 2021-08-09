use std::{
    mem, ptr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::channel,
        Arc,
    },
    thread,
    time::Duration,
};

use libc::{
    c_void, clockid_t, gettid, pid_t, pthread_self, pthread_t, sigaction, sigemptyset, sigevent,
    siginfo_t, timer_create, timer_settime, timer_t, SA_SIGINFO, SIGALRM, SIGEV_THREAD_ID,
};
use rusty_v8 as v8;

extern "C" {
    fn pthread_getcpuclockid(thread: pthread_t, clockid: &mut clockid_t) -> i32;
}

#[derive(Debug)]
pub struct Watchdog {
    status: Arc<AtomicUsize>,
    tid: pid_t,
}

enum WatchdogStatus {
    Initial,
    Starting,
    Started,
}

#[derive(Debug)]
pub struct WatchOptions {
    pub cpu_timeout: Duration,
}

impl Watchdog {
    pub fn new() -> Self {
        let mut watchdog = Watchdog {
            status: Arc::new(0.into()),
            tid: 0,
        };
        watchdog.start_watch_thread();
        watchdog
    }

    fn start_watch_thread(&mut self) {
        if let Err(_) = self.status.compare_exchange(
            WatchdogStatus::Initial as usize,
            WatchdogStatus::Starting as usize,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            ()
        }

        let (tx, rx) = channel::<pid_t>();
        thread::spawn(move || {
            Watchdog::setup_sigaction();
            tx.send(unsafe { gettid() }).unwrap();
            thread::park();
        });
        self.tid = rx.recv().unwrap();
        self.status
            .store(WatchdogStatus::Started as usize, Ordering::SeqCst);
    }

    fn setup_sigaction() {
        let mut act: sigaction = unsafe { mem::zeroed() };
        act.sa_flags = SA_SIGINFO;
        act.sa_sigaction = Watchdog::sigaction as usize;
        unsafe { sigaction(SIGALRM, &act, ptr::null_mut()) };
        unsafe {
            sigemptyset(&mut act.sa_mask);
            sigaction(SIGALRM, &act, ptr::null_mut());
        }
    }

    fn sigaction(_sig: i32, info: &mut siginfo_t, _ucontext: *mut c_void) {
        // TODO: check if IsolateHandle is freed?

        println!("terminating the Isolate");
        let handle = unsafe { &*(info.si_value().sival_ptr as *const v8::IsolateHandle) };
        handle.terminate_execution();
    }

    pub fn watch(&self, isolate_handle: &v8::IsolateHandle, options: WatchOptions) {
        assert_eq!(
            self.status.load(Ordering::SeqCst),
            WatchdogStatus::Started as usize
        );
        let mut clockid: clockid_t = unsafe { mem::zeroed() };
        unsafe { pthread_getcpuclockid(pthread_self(), &mut clockid) };

        let mut timer_id: timer_t = unsafe { mem::zeroed() };
        let mut sev: sigevent = unsafe { mem::zeroed() };
        sev.sigev_notify = SIGEV_THREAD_ID;
        sev.sigev_signo = SIGALRM;
        sev.sigev_notify_thread_id = self.tid;
        sev.sigev_value.sival_ptr = isolate_handle as *const _ as *mut c_void;
        let new_value = &mut unsafe { mem::zeroed::<libc::itimerspec>() };
        new_value.it_value.tv_sec = options.cpu_timeout.as_secs() as i64;
        new_value.it_value.tv_nsec = (options.cpu_timeout.as_nanos() % 1_000_000_000) as i64;
        unsafe {
            timer_create(clockid, &mut sev, &mut timer_id);
            timer_settime(timer_id, 0, new_value, ptr::null_mut());
        }
    }
}
