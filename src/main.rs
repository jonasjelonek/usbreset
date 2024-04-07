#[cfg(not(target_os = "linux"))]
compile_error!("only linux is supported");

/* Constants taken/adopted from Linux's usbdevice_fs.h */
/* Code inspired by Greg Kroah-Hartman's usbutils */

use std::env;
use std::fs;
use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::io::Result;

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
const USBSYSFS_PATH: &str = "/sys/bus/usb/devices";

const WHITESPACE_CHARS: [char; 3] = [ '\n', '\t', ' ' ];


#[derive(Debug)]
enum UsbDeviceIdentifier {
	BusDev { bus: u16, dev: u16},
	VendorProduct { vid: u16, pid: u16 },
	ProductName(String),
}

struct UsbDevFsEntry {
	bus: u16,
	dev: u16,
}

fn sysfs_attr_raw<P: AsRef<Path>>(dev: P, attr: &str) -> Result<String> {
	let mut path = PathBuf::from_str(USBSYSFS_PATH).unwrap();
	path.push(dev);
	path.push(attr);

	fs::read_to_string(path)
		.map(|mut s| {
			if s.ends_with(WHITESPACE_CHARS.as_slice()) {
				s.pop();
			}
			s
		})
}

fn sysfs_attr<T: FromStr, P: AsRef<Path>>(dev: P, attr: &str) -> Result<T> {
	sysfs_attr_raw(dev, attr)?
		.parse()
		.map_err(|_| ErrorKind::InvalidData.into())
}

fn find_device(identifier: UsbDeviceIdentifier) -> Result<UsbDevFsEntry> {
	for entry in std::fs::read_dir(USBSYSFS_PATH)? {
		let Ok(dir) = entry else { continue; };
		let dev_name = dir.file_name();

		let Ok(e_bus) = sysfs_attr::<u16, _>(dev_name.as_os_str(), "busnum") else { continue };
		let Ok(e_dev) = sysfs_attr::<u16, _>(dev_name.as_os_str(), "devnum") else { continue };

		match identifier {
			UsbDeviceIdentifier::BusDev { bus, dev } => {
				if e_bus == bus && e_dev == dev {
					return Ok(UsbDevFsEntry { bus: e_bus, dev: e_dev });
				}
			},
			UsbDeviceIdentifier::VendorProduct { vid, pid } => {
				let Ok(vid_str) = sysfs_attr_raw(&dev_name[..], "idVendor") else { continue };
				let Ok(pid_str) = sysfs_attr_raw(&dev_name[..], "idProduct") else { continue };

				let cur_vid = u16::from_str_radix(&vid_str[..], 16)
					.map_err(|_| ErrorKind::InvalidData)?;
				let cur_pid = u16::from_str_radix(&pid_str[..], 16)
					.map_err(|_| ErrorKind::InvalidData)?;

				if cur_vid == vid && cur_pid == pid {
					return Ok(UsbDevFsEntry { bus: e_bus, dev: e_dev });
				}
			},
			UsbDeviceIdentifier::ProductName(ref name) => {
				let Ok(cur_name) = sysfs_attr_raw(dev_name.as_os_str(), "product") else { continue };

				if cur_name == *name {
					return Ok(UsbDevFsEntry { bus: e_bus, dev: e_dev });
				}
			}
		}
	}

	Err(std::io::Error::from(ErrorKind::NotFound))
}

fn reset_device(usbdev: UsbDevFsEntry) -> Result<()> {
	let path = format!("{}/{:03}/{:03}", USBDEVFS_PATH, usbdev.bus, usbdev.dev);
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

fn main() -> Result<()> {
	println!("USB DEVICE RESET");

	let args = env::args().skip(1).collect::<Vec<String>>();
	if args.len() < 1 {
		println!("No usb device specified!");
		return Err(std::io::Error::from(ErrorKind::InvalidInput).into());
	}

	let identifier: UsbDeviceIdentifier;
	
	let (mut bus, mut dev) = (0, 0);
	let (mut vid_str, mut pid_str) = (String::new(), String::new());

	if scanf::sscanf!(&args[0], "{u16}/{u16}", bus, dev).is_ok() {
		identifier = UsbDeviceIdentifier::BusDev { bus, dev }
	} else if scanf::sscanf!(&args[0], "{string}:{string}", vid_str, pid_str).is_ok() {
		let vid = u16::from_str_radix(&vid_str[..], 16)
			.map_err(|_| ErrorKind::InvalidData)?;
		let pid = u16::from_str_radix(&pid_str[..], 16)
			.map_err(|_| ErrorKind::InvalidData)?;

		identifier = UsbDeviceIdentifier::VendorProduct { vid, pid };
	} else {
		let mut name = String::new();
		scanf::sscanf!(args[0].as_str(), "{string}", name)?;
		identifier = UsbDeviceIdentifier::ProductName(name);
	}

	let usbdev = find_device(identifier)?;
	Ok(reset_device(usbdev)?)
}