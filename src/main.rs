
/* Stuff taken/adopted from Linux's usbdevice_fs.h */

use std::env;
use std::fs;
use std::os::fd::AsRawFd;
use regex;

const _IOC_NRBITS: usize = 8;
const _IOC_TYPEBITS: usize = 8;
const _IOC_SIZEBITS: usize = 14;
const _IOC_DIRBITS: usize = 2;

const _IOC_NRMASK: usize = (1 << _IOC_NRBITS) - 1;
const _IOC_TYPEMASK: usize = (1 << _IOC_TYPEBITS) - 1;
const _IOC_SIZEMASK: usize = (1 << _IOC_SIZEBITS) - 1;
const _IOC_DIRMASK: usize = (1 << _IOC_DIRBITS) - 1;

const _IOC_NRSHIFT: usize = 0;
const _IOC_TYPESHIFT: usize = _IOC_NRSHIFT + _IOC_NRBITS;
const _IOC_SIZESHIFT: usize = _IOC_TYPESHIFT + _IOC_TYPEBITS;
const _IOC_DIRSHIFT: usize = _IOC_SIZESHIFT + _IOC_SIZEBITS;

const _IOC_NONE: usize = 0;

macro_rules! _IOC {
	($dir:expr, $type:expr, $nr:expr, $size:expr) => {
		(
			(($dir) << _IOC_DIRSHIFT) |
				(($type) << _IOC_TYPESHIFT) |
				(($nr) << _IOC_NRSHIFT) |
				(($size) << _IOC_SIZESHIFT)
		)
	};
}

macro_rules! _IO {
	($type:expr, $nr:expr) => {
		_IOC!(_IOC_NONE, ($type), ($nr), 0)
	};
}

const USBDEVFS_RESET: usize = _IO!(b'U' as usize, 20_usize);
const USBDEVFS_PATH: &str = "/dev/bus/usb/";

/* End stuff */

fn main() -> std::io::Result<()> {
	println!("USB DEVICE RESET");

	let args = env::args().skip(1).collect::<Vec<String>>();
	if args.len() < 1 {
		println!("No usb device specified!");
		return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
	}

	let usb_regex = regex::Regex::new("^[0-9a-fA-F]{3}:[0-9a-fA-F]{3}$").unwrap();
	if !usb_regex.is_match(&args[0]) {
		println!("Invalid usb device specified!");
		return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
	}
	let usb = args[0].split(":").collect::<Vec<&str>>();

	let path = format!("{}/{}/{}", USBDEVFS_PATH, usb[0], usb[1]);
	let dev_file = fs::OpenOptions::new().write(true).open(path)?;

	#[cfg(target_env = "musl")]
	let res = unsafe {
		libc::ioctl(dev_file.as_raw_fd() as libc::c_int, USBDEVFS_RESET as libc::c_int)
	};
	#[cfg(not(target_env = "musl"))]
	let res = unsafe {
		libc::ioctl(dev_file.as_raw_fd() as libc::c_int, USBDEVFS_RESET as libc::c_ulong)
	};

	if res == 0 {
		println!("USB reset successful");
		Ok(())
	} else {
		println!("USB reset failed: {res}");
		Err(std::io::Error::from(std::io::ErrorKind::Other))
	}
}
