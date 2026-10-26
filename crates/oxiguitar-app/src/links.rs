// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Opening a web address in the system's browser, only when the user clicks
//! a link (Help → About's donation button; the player's link to better
//! sounds, M4). OxiGuitar itself never connects anywhere: the browser does,
//! as a program of its own.

use std::process::{Command, Stdio};

/// Where donations go: Mateus's PayPal page (the README's button).
pub(crate) const DONATE: &str = "https://www.paypal.com/donate/?business=4QYDZY4RP8V8L&no_recurring=0&item_name=Thanks+heaps!+You+rock!&currency_code=AUD";

/// The system's command that opens an address in the default browser.
fn opener(url: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        // url.dll takes the address as it is, & and all (cmd's `start`
        // would split it there).
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    }
    #[cfg(target_os = "macos")]
    {
        let mut c = Command::new("open");
        c.arg(url);
        c
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    }
}

/// Opens `url` in the browser. Only `https://` addresses are opened, so
/// nothing else (a file, a program) can ever be started this way.
pub(crate) fn open(url: &str) -> Result<(), String> {
    if !oxiguitar_host::is_https(url) {
        return Err(format!("{url} is not a web address"));
    }
    let mut child = opener(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("The browser couldn't be started ({e}). The address is {url}"))?;
    // Wait for the opener (it hands the address over and ends) on a thread
    // of its own, so the window never waits and no zombie is left behind.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_addresses_are_opened() {
        assert!(open("file:///etc/passwd").is_err());
        assert!(open("/usr/bin/true").is_err());
        assert!(open("http://example.org").is_err());
    }

    #[test]
    fn the_donation_address_is_paypal_over_https() {
        assert!(DONATE.starts_with("https://www.paypal.com/donate/"));
        assert!(!DONATE.contains(' '));
    }
}
