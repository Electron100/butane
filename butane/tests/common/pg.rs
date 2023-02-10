use butane::db::{Backend, Connection, ConnectionSpec};
use once_cell::sync::Lazy;
use std::io::{BufRead, BufReader, Read, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::process::{ChildStderr, Command, Stdio};
use std::sync::Mutex;
use uuid_for_test::Uuid;

pub fn pg_connection() -> (Connection, PgSetupData) {
    let backend = butane::db::get_backend("pg").unwrap();
    let data = pg_setup();
    (backend.connect(&pg_connstr(&data)).unwrap(), data)
}

pub fn pg_connspec() -> (ConnectionSpec, PgSetupData) {
    let data = pg_setup();
    (
        ConnectionSpec::new(butane::db::pg::BACKEND_NAME, pg_connstr(&data)),
        data,
    )
}

struct PgServerState {
    pub dir: PathBuf,
    pub sockdir: PathBuf,
    pub proc: std::process::Child,
    // stderr from the child process
    pub stderr: BufReader<ChildStderr>,
}
impl Drop for PgServerState {
    fn drop(&mut self) {
        self.proc.kill().ok();
        let mut buf = String::new();
        self.stderr.read_to_string(&mut buf).unwrap();
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

pub struct PgSetupData {
    pub connstr: String,
}

fn create_tmp_server() -> PgServerState {
    eprintln!("create tmp server");
    // create a temporary directory
    let dir = std::env::current_dir()
        .unwrap()
        .join("tmp_pg")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();

    // Run initdb to create a postgres cluster in our temporary director
    let output = Command::new("initdb")
        .arg("-D")
        .arg(&dir)
        .arg("-U")
        .arg("postgres")
        .output()
        .expect("failed to run initdb");
    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("postgres initdb failed")
    }

    let sockdir = dir.join("socket");
    std::fs::create_dir(&sockdir).unwrap();

    // Run postgres to actually create the server
    let mut proc = Command::new("postgres")
        .arg("-c")
        .arg("logging_collector=false")
        .arg("-D")
        .arg(&dir)
        .arg("-k")
        .arg(&sockdir)
        .arg("-h")
        .arg("")
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run postgres");
    let mut buf = String::new();
    let mut stderr = BufReader::new(proc.stderr.take().unwrap());
    loop {
        buf.clear();
        stderr.read_line(&mut buf).unwrap();
        if buf.contains("ready to accept connections") {
            break;
        }
        if proc.try_wait().unwrap().is_some() {
            buf.clear();
            stderr.read_to_string(&mut buf).unwrap();
            eprint!("{buf}");
            panic!("postgres process died");
        }
    }
    eprintln!("created tmp server!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    unsafe {
        // Try to delete all the pg files when the process exits
        libc::atexit(proc_teardown);
    }
    PgServerState {
        dir,
        sockdir,
        proc,
        stderr,
    }
}

extern "C" fn proc_teardown() {
    drop(TMP_SERVER.deref().lock().unwrap().take());
}

static TMP_SERVER: Lazy<Mutex<Option<PgServerState>>> =
    Lazy::new(|| Mutex::new(Some(create_tmp_server())));

pub fn pg_setup() -> PgSetupData {
    eprintln!("pg_setup");
    // By default we set up a temporary, local postgres server just
    // for this test. This can be overridden by the environment
    // variable BUTANE_PG_CONNSTR
    let connstr = match std::env::var("BUTANE_PG_CONNSTR") {
        Ok(connstr) => connstr,
        Err(_) => {
            let server_mguard = &TMP_SERVER.deref().lock().unwrap();
            let server: &PgServerState = server_mguard.as_ref().unwrap();
            let host = server.sockdir.to_str().unwrap();
            format!("host={host} user=postgres")
        }
    };
    let new_dbname = format!("butane_test_{}", Uuid::new_v4().simple());
    eprintln!("new db is `{}`", &new_dbname);

    let mut conn = butane::db::connect(&ConnectionSpec::new("pg", &connstr)).unwrap();
    conn.execute(format!("CREATE DATABASE {new_dbname};"))
        .unwrap();

    let connstr = format!("{connstr} dbname={new_dbname}");
    PgSetupData { connstr }
}
pub fn pg_teardown(_data: PgSetupData) {
    // All the work is done by the drop implementation
}

pub fn pg_connstr(data: &PgSetupData) -> String {
    data.connstr.clone()
}
