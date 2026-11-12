use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
struct App {
    child: Child,
    address: String,
    token: String,
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl App {
    fn start(dir: &PathBuf) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gridepos-cotizaciones"))
            .env("GRIDEPOS_DATA_DIR", dir)
            .env("GRIDEPOS_NO_BROWSER", "1")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut r = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        let address = loop {
            line.clear();
            assert!(r.read_line(&mut line).unwrap() > 0, "Server did not start");
            if let Some(url) = line.strip_prefix("Aplicación local: http://") {
                break url.trim().to_string();
            }
        };
        let mut app = Self {
            child,
            address,
            token: String::new(),
        };
        let (_, index) = app.call("GET", "/", None, &[]);
        app.token = index
            .split("name=\"local-token\" content=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string();
        app
    }
    fn call(
        &self,
        method: &str,
        path: &str,
        data: Option<&Value>,
        extra: &[(&str, &str)],
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let body = data.map(Value::to_string).unwrap_or_default();
        let host = extra
            .iter()
            .find(|(k, _)| *k == "Host")
            .map(|(_, v)| *v)
            .unwrap_or(&self.address);
        write!(stream,"{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",method,path,host,body.len()).unwrap();
        if !self.token.is_empty() {
            write!(stream, "X-Local-Token: {}\r\n", self.token).unwrap();
        }
        for (key, val) in extra {
            if *key != "Host" {
                write!(stream, "{}: {}\r\n", key, val).unwrap();
            }
        }
        write!(stream, "\r\n{}", body).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, body.to_string())
    }
}
#[test]
fn offline_storage_backup_and_security() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("gridepos-test-{}-{}", std::process::id(), stamp));
    fs::create_dir_all(&dir).unwrap();
    let mut app = App::start(&dir);
    let token = app.token.clone();
    app.token.clear();
    assert_eq!(app.call("GET", "/api/quotes", None, &[]).0, 401);
    app.token = token;
    assert_eq!(
        app.call(
            "GET",
            "/api/quotes",
            None,
            &[("Origin", "https://example.org")]
        )
        .0,
        403
    );
    assert_eq!(
        app.call("GET", "/api/quotes", None, &[("Host", "example.org")])
            .0,
        403
    );
    let mut quote = json!({"number":"TEST-OFFLINE","date":"2026-10-14","company":"GRIDEPOS, S.R.L.","rnc":"1-33-79278-8","client":"Cliente de prueba","option_a":"Offline","option_b":"Online","equipment":[{"description":"Lector","quantity":"2","price":"1500","tax":"18"}],"recurring":[{"description":"Mensual","quantity":1,"price":3000,"tax":0,"option":"b","period":"Mensual"},{"description":"Anual","quantity":1,"price":12000,"tax":0,"option":"b","period":"Anual"}]});
    let (code, b) = app.call("POST", "/api/quotes", Some(&quote), &[]);
    assert_eq!(code, 200, "{}", b);
    let saved: Value = serde_json::from_str(&b).unwrap();
    let id = saved["id"].as_str().unwrap().to_string();
    quote["revision"] = saved["revision"].clone();
    let (code, b) = app.call("PUT", &format!("/api/quotes/{}", id), Some(&quote), &[]);
    assert_eq!(code, 200);
    let saved: Value = serde_json::from_str(&b).unwrap();
    assert_eq!(
        app.call("PUT", &format!("/api/quotes/{}", id), Some(&quote), &[])
            .0,
        409
    );
    quote["revision"] = saved["revision"].clone();
    let backup: Value = serde_json::from_str(&app.call("GET", "/api/backup", None, &[]).1).unwrap();
    let merged: Value =
        serde_json::from_str(&app.call("POST", "/api/restore", Some(&backup), &[]).1).unwrap();
    assert_eq!(merged["restored"], 0);
    assert_eq!(merged["skipped"], 1);
    let mut new_backup = backup.clone();
    new_backup["quotes"][0]["id"] = json!("0123456789abcdef0123456789abcdef0123456789abcdef");
    let merged: Value =
        serde_json::from_str(&app.call("POST", "/api/restore", Some(&new_backup), &[]).1).unwrap();
    assert_eq!(merged["restored"], 1);
    let mut bad = new_backup.clone();
    bad["quotes"][0]["quote"]["equipment"][0]["tax"] = json!(101);
    assert_eq!(app.call("POST", "/api/restore", Some(&bad), &[]).0, 400);
    let calc: Value =
        serde_json::from_str(&app.call("POST", "/api/calculate", Some(&quote), &[]).1).unwrap();
    assert_eq!(
        calc["a"]["initial"]
            .as_str()
            .unwrap()
            .parse::<rust_decimal::Decimal>()
            .unwrap(),
        rust_decimal::Decimal::from(3540)
    );
    assert_eq!(calc["b"]["periods"]["Mensual"], "3000");
    assert_eq!(calc["b"]["periods"]["Anual"], "12000");
    drop(app);
    let app = App::start(&dir);
    let (code, b) = app.call("GET", &format!("/api/quotes/{}", id), None, &[]);
    assert_eq!(code, 200);
    let recovered: Value = serde_json::from_str(&b).unwrap();
    assert_eq!(recovered["client"], "Cliente de prueba");
    assert_eq!(recovered["revision"], 2);
    drop(app);
    fs::remove_dir_all(dir).unwrap();
}
