use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::sync::{Mutex, OnceLock};

fn urandom() -> &'static Mutex<File> {
    static FILE: OnceLock<Mutex<File>> = OnceLock::new();
    FILE.get_or_init(|| {
        let file = File::open("/dev/urandom").expect("open /dev/urandom");
        Mutex::new(file)
    })
}

pub(crate) fn fill_random(output: &mut [u8]) -> Result<(), Error> {
    if output.is_empty() {
        return Ok(());
    }
    let mut file = urandom().lock().expect("urandom mutex poisoned");
    file.read_exact(output).map_err(|error| {
        if error.kind() == ErrorKind::UnexpectedEof {
            Error::new(ErrorKind::UnexpectedEof, "short read from /dev/urandom")
        } else {
            error
        }
    })
}
