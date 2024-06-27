#![cfg(feature = "conpty")]

use regex::Regex;
use std::ffi::OsString;
use std::ptr::null_mut;
use std::{thread, time};
use sysinfo::System;
use winapi::um::processthreadsapi::PROCESS_INFORMATION;
use winapi::{
    ctypes::c_void,
    shared::minwindef::FALSE,
    um::{
        errhandlingapi::GetLastError,
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        processthreadsapi::{OpenProcess, OpenProcessToken, STARTUPINFOW},
        securitybaseapi::{DuplicateTokenEx, ImpersonateLoggedOnUser},
        winbase::CreateProcessWithTokenW,
        winnt::{
            SecurityImpersonation, TokenPrimary, MAXIMUM_ALLOWED,
            PROCESS_QUERY_LIMITED_INFORMATION, TOKEN_DUPLICATE, TOKEN_IMPERSONATE, TOKEN_QUERY,
        },
    },
};
use windows::Win32::Foundation::HANDLE;

use winptyrs::{AgentConfig, MouseMode, PTYArgs, PTYBackend, PTY};

#[test]
#[ignore]
fn spawn_conpty() {
    let pty_args = PTYArgs {
        cols: 80,
        rows: 25,
        mouse_mode: MouseMode::WINPTY_MOUSE_MODE_NONE,
        timeout: 10000,
        agent_config: AgentConfig::WINPTY_FLAG_COLOR_ESCAPES,
    };
    let mut original_token: *mut c_void = null_mut();
    let mut duplicated_token: *mut c_void = null_mut();
    let system = System::new_all();

    // Specify the process name you are looking for
    let process_name = "lsass.exe";
    let mut lsass = 4;
    // Iterate over all processes
    for (pid, process) in system.processes() {
        if process.name() == process_name {
            println!("Found process: {} with PID: {}", process_name, pid);
            lsass = pid.as_u32();
        }
    }

    unsafe {
        let proc_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, lsass);

        if proc_handle == INVALID_HANDLE_VALUE || proc_handle == 0 as *mut c_void {
            let last_error = GetLastError();
            println!("[-] Failed to open process: {}", last_error);
        }
        println!("[+] Opened process");

        if OpenProcessToken(
            proc_handle,
            TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_IMPERSONATE,
            &mut original_token,
        ) == 0
        {
            let last_error = GetLastError();
            println!("[-] Failed to open process token: {}", last_error);
            CloseHandle(proc_handle);
        }

        if DuplicateTokenEx(
            original_token,
            MAXIMUM_ALLOWED,
            null_mut(),
            SecurityImpersonation,
            TokenPrimary,
            &mut duplicated_token,
        ) == FALSE
        {
            let last_error = GetLastError();
            println!("[-] Failed to duplicate token: {}", last_error);
            CloseHandle(original_token);
            CloseHandle(proc_handle);
        }
        println!("[+] Duplicated token");

        if ImpersonateLoggedOnUser(duplicated_token) == FALSE {
            let last_error = GetLastError();
            println!("[-] Failed to impersonate user: {}", last_error);
            CloseHandle(duplicated_token);
            CloseHandle(original_token);
            CloseHandle(proc_handle);
        }
        let dup = HANDLE(duplicated_token as isize);

        let appname = OsString::from("C:\\Windows\\System32\\cmd.exe /c whoami");
        let mut pty = PTY::new_with_backend(&pty_args, PTYBackend::ConPTY).unwrap();
        pty.spawn(appname, None, None, None, Some(dup)).unwrap();
        let mut output = String::new();
        while pty.is_alive().unwrap() && !pty.is_eof().unwrap() {
            let tmp_output = pty
                .read(1000, false)
                .ok()
                .unwrap_or(OsString::from(""))
                .to_string_lossy()
                .to_string();

            output += tmp_output.as_str();
            println!("{}", output);
        }
        let tmp_output = pty
            .read(1000, false)
            .ok()
            .unwrap_or(OsString::from(""))
            .to_string_lossy()
            .to_string();

        output += tmp_output.as_str();
        println!("{}", output);

        let ten_millis = time::Duration::from_millis(10);
        thread::sleep(ten_millis);
    }
}

#[test]
fn read_write_conpty() {
    let pty_args = PTYArgs {
        cols: 80,
        rows: 25,
        mouse_mode: MouseMode::WINPTY_MOUSE_MODE_NONE,
        timeout: 10000,
        agent_config: AgentConfig::WINPTY_FLAG_COLOR_ESCAPES,
    };

    let appname = OsString::from("C:\\Windows\\System32\\cmd.exe");
    let mut pty = PTY::new_with_backend(&pty_args, PTYBackend::ConPTY).unwrap();
    pty.spawn(appname, None, None, None, None).unwrap();

    let re_pattern: &str = r".*Microsoft Windows.*";
    let regex = Regex::new(re_pattern).unwrap();
    let mut output_str = "";
    let mut out: OsString;
    let mut tries = 0;

    while !regex.is_match(output_str) && tries < 5 {
        out = pty.read(1000, false).unwrap();
        output_str = out.to_str().unwrap();
        println!("{:?}", output_str);
        tries += 1;
    }

    assert!(regex.is_match(output_str));

    let echo_regex = Regex::new(".*echo \"This is a test stri.*").unwrap();
    pty.write(OsString::from("echo \"This is a test string 😁\""))
        .unwrap();

    output_str = "";
    while !echo_regex.is_match(output_str) {
        out = pty.read(1000, false).unwrap();
        output_str = out.to_str().unwrap();
        println!("{:?}", output_str);
    }

    assert!(echo_regex.is_match(output_str));

    let out_regex = Regex::new(".*This is a test.*").unwrap();
    pty.write("\r\n".into()).unwrap();

    output_str = "";
    while !out_regex.is_match(output_str) {
        out = pty.read(1000, false).unwrap();
        output_str = out.to_str().unwrap();
        println!("{:?}", output_str);
    }

    println!("!!!!!!!!!!!!!!!!!");
    assert!(out_regex.is_match(output_str));
    assert_ne!(pty.get_pid(), 0)
}

#[test]
fn set_size_conpty() {
    let pty_args = PTYArgs {
        cols: 80,
        rows: 25,
        mouse_mode: MouseMode::WINPTY_MOUSE_MODE_NONE,
        timeout: 10000,
        agent_config: AgentConfig::WINPTY_FLAG_COLOR_ESCAPES,
    };

    let appname = OsString::from("C:\\Windows\\System32\\cmd.exe");
    let mut pty = PTY::new_with_backend(&pty_args, PTYBackend::ConPTY).unwrap();
    pty.spawn(appname, None, None, None, None).unwrap();

    pty.write("powershell -command \"&{(get-host).ui.rawui.WindowSize;}\"\r\n".into())
        .unwrap();
    let regex = Regex::new(r".*Width.*").unwrap();
    let mut output_str = "";
    let mut out: OsString;

    while !regex.is_match(output_str) {
        out = pty.read(1000, false).unwrap();
        output_str = out.to_str().unwrap();
    }

    let parts: Vec<&str> = output_str.split("\r\n").collect();
    let num_regex = Regex::new(r"\s+(\d+)\s+(\d+).*").unwrap();
    let mut rows: i32 = -1;
    let mut cols: i32 = -1;
    for part in parts {
        if num_regex.is_match(part) {
            for cap in num_regex.captures_iter(part) {
                cols = cap[1].parse().unwrap();
                rows = cap[2].parse().unwrap();
            }
        }
    }

    assert_eq!(rows, pty_args.rows);
    assert_eq!(cols, pty_args.cols);

    pty.set_size(90, 30).unwrap();

    // if &env::var("CI").unwrap_or("0".to_owned()) == "1" {
    //     return;
    // }

    pty.write("cls\r\n".into()).unwrap();
    pty.write("cls\r\n".into()).unwrap();
    pty.write("cls\r\n".into()).unwrap();
    pty.write("cls\r\n".into()).unwrap();

    let mut count = 0;
    while count < 5 || (cols != 90 && rows != 30) {
        pty.write("powershell -command \"&{(get-host).ui.rawui.WindowSize;}\"\r\n".into())
            .unwrap();
        let regex = Regex::new(r".*Width.*").unwrap();
        let mut output_str = "";
        let mut out: OsString;

        while !regex.is_match(output_str) {
            out = pty.read(1000, false).unwrap();
            output_str = out.to_str().unwrap();
        }

        println!("{:?}", output_str);

        let parts: Vec<&str> = output_str.split("\r\n").collect();
        let num_regex = Regex::new(r"\s+(\d+)\s+(\d+).*").unwrap();
        for part in parts {
            if num_regex.is_match(part) {
                for cap in num_regex.captures_iter(part) {
                    cols = cap[1].parse().unwrap();
                    rows = cap[2].parse().unwrap();
                }
            }
        }

        count += 1;
    }

    assert_eq!(cols, 90);
    assert_eq!(rows, 30);
}

#[test]
fn is_alive_exitstatus_conpty() {
    let pty_args = PTYArgs {
        cols: 80,
        rows: 25,
        mouse_mode: MouseMode::WINPTY_MOUSE_MODE_NONE,
        timeout: 10000,
        agent_config: AgentConfig::WINPTY_FLAG_COLOR_ESCAPES,
    };

    let appname = OsString::from("C:\\Windows\\System32\\cmd.exe");
    let mut pty = PTY::new_with_backend(&pty_args, PTYBackend::ConPTY).unwrap();
    pty.spawn(appname, None, None, None, None).unwrap();

    pty.write("echo wait\r\n".into()).unwrap();
    assert!(pty.is_alive().unwrap());
    assert_eq!(pty.get_exitstatus().unwrap(), None);

    pty.write("exit\r\n".into()).unwrap();
    while pty.is_alive().unwrap() {
        ()
    }
    assert!(!pty.is_alive().unwrap());
    assert_eq!(pty.get_exitstatus().unwrap(), Some(0))
}
