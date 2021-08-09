use std::{
    mem, ptr,
    sync::atomic::{self, AtomicUsize},
    thread,
};

use libc::{
    clock_gettime, clockid_t, gettid, pthread_self, pthread_t, sigaction, sigemptyset, sigevent,
    timer_create, timer_settime, timer_t, timespec, SA_SIGINFO, SIGALRM, SIGEV_THREAD_ID,
};

static STOPPING: AtomicUsize = AtomicUsize::new(0);

fn watchdog(thread_clockid: clockid_t) {
    let mut act: sigaction = unsafe { mem::zeroed() };
    act.sa_flags = SA_SIGINFO;
    act.sa_sigaction = watchdog_callback as usize;
    unsafe { sigaction(SIGALRM, &act, ptr::null_mut()) };
    unsafe {
        sigemptyset(&mut act.sa_mask);
        sigaction(SIGALRM, &act, ptr::null_mut());
    }

    let mut timer_id: timer_t = unsafe { mem::zeroed() };
    let mut sev: sigevent = unsafe { mem::zeroed() };
    sev.sigev_notify = SIGEV_THREAD_ID;
    sev.sigev_signo = SIGALRM;
    sev.sigev_notify_thread_id = unsafe { gettid() };
    let new_value = &mut unsafe { mem::zeroed::<libc::itimerspec>() };
    new_value.it_value.tv_nsec = 20_000_000;
    unsafe {
        timer_create(thread_clockid, &mut sev, &mut timer_id);
        timer_settime(timer_id, 0, new_value, ptr::null_mut());
    }

    thread::park();
}

extern "C" fn watchdog_callback() {
    println!("watchdog_callback called!");
    STOPPING.store(1, atomic::Ordering::SeqCst)
}

extern "C" {
    fn pthread_getcpuclockid(thread: pthread_t, clockid: &mut clockid_t) -> i32;
}

fn main() {
    let mut clockid: clockid_t = unsafe { mem::zeroed() };
    unsafe { pthread_getcpuclockid(pthread_self(), &mut clockid) };
    thread::spawn(move || {
        watchdog(clockid);
    });

    let mut start: timespec = unsafe { mem::zeroed() };
    unsafe { clock_gettime(clockid, &mut start) };

    while STOPPING.load(atomic::Ordering::SeqCst) == 0 {}

    let mut end: timespec = unsafe { mem::zeroed() };
    unsafe { clock_gettime(clockid, &mut end) };
    println!(
        "{}ms",
        (end.tv_sec - start.tv_sec) * 1000 + (end.tv_nsec - start.tv_nsec) / 1_000_000
    );
}
