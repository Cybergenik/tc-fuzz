use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const PYTHON_SERVER: &str = r#"
import socket, signal, sys, os, ctypes

# Die when parent process exits.
if sys.platform == 'linux':
    ctypes.CDLL("libc.so.6").prctl(1, signal.SIGTERM)

signal.signal(signal.SIGALRM, lambda s, f: (_ for _ in ()).throw(TimeoutError()))

sock = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
sock.bind(sys.argv[1])

while True:
    data, client_addr = sock.recvfrom(4096)
    signal.setitimer(signal.ITIMER_REAL, 1.0)
    try:
        result = f'{float(eval(data.decode())):.17g}'
    except:
        result = "ERR"
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
    sock.sendto(result.encode(), client_addr)
"#;

pub struct PythonOracle {
    child: Child,
    server_path: PathBuf,
    client_path: PathBuf,
    sock: UnixDatagram,
}

impl PythonOracle {
    pub fn new() -> Result<Self, String> {
        let pid = std::process::id();
        let server_path = PathBuf::from(format!("/tmp/tc_calc_oracle_{pid}.sock"));
        let client_path = PathBuf::from(format!("/tmp/tc_calc_client_{pid}.sock"));
        let _ = std::fs::remove_file(&server_path);
        let _ = std::fs::remove_file(&client_path);

        let child = Command::new("python3")
            .arg("-c")
            .arg(PYTHON_SERVER)
            .arg(&server_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("python3 not found: {e}"))?;

        // wait for server socket to appear.
        for _ in 0..50 {
            if server_path.exists() {
                let sock = UnixDatagram::bind(&client_path)
                    .map_err(|e| format!("client bind: {e}"))?;
                sock.connect(&server_path)
                    .map_err(|e| format!("connect: {e}"))?;
                return Ok(Self { child, server_path, client_path, sock });
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Err("python oracle did not start in time".into())
    }

    pub fn eval(&mut self, expr: &str) -> Result<f64, ()> {
        self.sock.send(expr.as_bytes()).map_err(|_| ())?;
        let mut buf = [0u8; 64];
        let n = self.sock.recv(&mut buf).map_err(|_| ())?;
        let response = std::str::from_utf8(&buf[..n]).map_err(|_| ())?;
        if response == "ERR" {
            return Err(());
        }
        response.parse::<f64>().map_err(|_| ())
    }
}

impl Drop for PythonOracle {
    fn drop(&mut self) {
        let pid = self.child.id() as i32;
        unsafe { libc::kill(pid, libc::SIGTERM); }
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.server_path);
        let _ = std::fs::remove_file(&self.client_path);
    }
}
