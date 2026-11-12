mod invoice;
use chrono::{Local, NaiveDate};
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{env, fs, io::Read, path::PathBuf, process::Command, str::FromStr};
use tiny_http::{Header, Request, Response, Server, StatusCode};

const INDEX: &str = include_str!("../static/index.html");
const JS: &str = include_str!("../static/app.js");
const CSS: &str = include_str!("../static/style.css");
const LOGO: &[u8] = include_bytes!("../static/logo.png");
const MAX_BODY: u64 = 8 * 1024 * 1024;

type AppResult<T> = Result<T, String>;
fn decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
    let v = Value::deserialize(d)?;
    let s = match v {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => return Err(serde::de::Error::custom("Número inválido")),
    };
    Decimal::from_str(&s).map_err(serde::de::Error::custom)
}
fn one() -> Decimal {
    Decimal::ONE
}
fn both() -> String {
    "both".into()
}
fn monthly() -> String {
    "Mensual".into()
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Item {
    description: String,
    #[serde(default = "one", deserialize_with = "decimal")]
    quantity: Decimal,
    #[serde(default, deserialize_with = "decimal")]
    price: Decimal,
    #[serde(default, deserialize_with = "decimal")]
    tax: Decimal,
    #[serde(default = "both")]
    option: String,
    #[serde(default = "monthly")]
    period: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Quote {
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_option")]
    invoice_option: String,
    #[serde(default)]
    client_rnc: String,
    #[serde(default)]
    client_address: String,
    #[serde(default)]
    payment_terms: String,
    #[serde(default)]
    fiscal_reference: String,
    #[serde(default)]
    due_date: Option<NaiveDate>,
    #[serde(default)]
    source_number: String,
    number: String,
    date: NaiveDate,
    company: String,
    rnc: String,
    #[serde(default)]
    client: String,
    #[serde(default)]
    contact: String,
    #[serde(default)]
    subtitle: String,
    option_a: String,
    option_b: String,
    #[serde(default)]
    equipment: Vec<Item>,
    #[serde(default)]
    services: Vec<Item>,
    #[serde(default)]
    recurring: Vec<Item>,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    revision: i64,
}
fn default_kind() -> String {
    "quote".into()
}
fn default_option() -> String {
    "a".into()
}
fn limited(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.contains('\0')
}
impl Quote {
    fn validate(&self) -> AppResult<()> {
        if !["quote", "invoice"].contains(&self.kind.as_str())
            || !["a", "b"].contains(&self.invoice_option.as_str())
        {
            return Err("Tipo de documento o alternativa de factura inválida.".into());
        }
        if !limited(&self.client_address, 1000)
            || !limited(&self.payment_terms, 1000)
            || !limited(&self.fiscal_reference, 100)
            || !limited(&self.client_rnc, 100)
            || !limited(&self.source_number, 100)
        {
            return Err("Revisa los campos del cliente, pago y referencia fiscal.".into());
        }

        if self.number.trim().is_empty() || !limited(&self.number, 100) {
            return Err("Indica un número de documento (máximo 100 caracteres).".into());
        }
        for s in [
            &self.company,
            &self.rnc,
            &self.client,
            &self.contact,
            &self.option_a,
            &self.option_b,
        ] {
            if !limited(s, 300) {
                return Err("Un campo supera los 300 caracteres.".into());
            }
        }
        if self.company.trim().is_empty()
            || self.option_a.trim().is_empty()
            || self.option_b.trim().is_empty()
        {
            return Err("Indica la empresa y los nombres de ambas alternativas.".into());
        }
        if !limited(&self.subtitle, 1000) || !limited(&self.notes, 10000) {
            return Err("La presentación o las notas son demasiado largas.".into());
        }
        for group in [&self.equipment, &self.services, &self.recurring] {
            if group.len() > 100 {
                return Err("Máximo 100 conceptos por sección.".into());
            }
            for i in group {
                if !limited(&i.description, 1500)
                    || i.quantity <= Decimal::ZERO
                    || i.quantity > Decimal::from(100000)
                    || i.price < Decimal::ZERO
                    || i.price > Decimal::from(100000000)
                    || i.tax < Decimal::ZERO
                    || i.tax > Decimal::from(100)
                    || i.quantity.scale() > 2
                    || i.price.scale() > 2
                    || i.tax.scale() > 2
                {
                    return Err("Revisa las cantidades, precios e ITBIS: valores positivos, hasta dos decimales y tasa entre 0 y 100%.".into());
                }
                if !["both", "a", "b"].contains(&i.option.as_str())
                    || !["Mensual", "Trimestral", "Anual", "Semanal"].contains(&i.period.as_str())
                {
                    return Err("Alternativa o periodicidad inválida.".into());
                }
            }
        }
        Ok(())
    }
}
fn cents(n: Decimal) -> Decimal {
    n.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}
fn line(i: &Item) -> (Decimal, Decimal, Decimal) {
    let b = cents(i.quantity * i.price);
    let t = cents(b * i.tax / Decimal::from(100));
    (b, t, b + t)
}
fn applies(i: &Item, opt: &str) -> bool {
    i.option == "both" || i.option == opt
}
fn sum(items: &[Item], opt: &str) -> Decimal {
    items
        .iter()
        .filter(|i| applies(i, opt))
        .map(|i| line(i).2)
        .sum()
}
fn totals(q: &Quote) -> Value {
    let mut out = json!({});
    for opt in ["a", "b"] {
        let eq = sum(&q.equipment, opt);
        let sv = sum(&q.services, opt);
        let mut recurring = json!({});
        for period in ["Mensual", "Trimestral", "Anual", "Semanal"] {
            let n: Decimal = q
                .recurring
                .iter()
                .filter(|i| applies(i, opt) && i.period == period)
                .map(|i| line(i).2)
                .sum();
            recurring[period] = json!(n.to_string());
        }
        let base: Decimal = q
            .equipment
            .iter()
            .chain(q.services.iter())
            .filter(|i| applies(i, opt))
            .map(|i| line(i).0)
            .sum();
        let tax: Decimal = q
            .equipment
            .iter()
            .chain(q.services.iter())
            .filter(|i| applies(i, opt))
            .map(|i| line(i).1)
            .sum();
        out[opt] = json!({"base":base.to_string(),"tax":tax.to_string(),"equipment":eq.to_string(),"services":sv.to_string(),"initial":(eq+sv).to_string(),"periods":recurring});
    }
    out
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn money(n: Decimal) -> String {
    let s = format!("{:.2}", cents(n));
    let (whole, frac) = s.split_once('.').unwrap_or((&s, "00"));
    let mut result = String::new();
    for (ix, c) in whole.chars().enumerate() {
        if ix > 0 && (whole.len() - ix) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    format!("RD$ {}.{}", result, frac)
}
fn format_date(d: NaiveDate) -> String {
    use chrono::Datelike;
    let months = [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ];
    format!(
        "{} de {} de {}",
        d.day(),
        months[d.month0() as usize],
        d.year()
    )
}
fn row(values: &[String]) -> String {
    format!(
        "<tr>{}</tr>",
        values
            .iter()
            .map(|v| format!("<td>{}</td>", v))
            .collect::<String>()
    )
}
fn table(head: &[&str], rows: String, widths: &str) -> String {
    format!(
        "<table>{}<thead><tr>{}</tr></thead><tbody>{}</tbody></table>",
        widths,
        head.iter()
            .map(|s| format!("<th>{}</th>", esc(s)))
            .collect::<String>(),
        if rows.is_empty() {
            format!("<tr><td colspan='{}'>Sin conceptos</td></tr>", head.len())
        } else {
            rows
        }
    )
}
fn print_html(q: &Quote) -> String {
    if q.kind == "invoice" {
        return invoice::print_invoice(q);
    }

    let mut s = format!(
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{}</title><link rel="stylesheet" href="/print.css"></head><body><nav class="print-tools"><button id="print">Imprimir / guardar PDF</button><span>Selecciona A4, escala 100% y desactiva los encabezados y pies del navegador.</span></nav><article><header><img src="/logo.png" alt="GridePOS"><div><h1>COTIZACIÓN COMERCIAL</h1><b>No. {}</b><p>Fecha: {}</p></div></header><p class="company"><b>{} &nbsp; | &nbsp; RNC: {}</b></p><p class="subtitle">{}</p><h2>DATOS DEL CLIENTE</h2><p class="client">Cliente / empresa: {} &nbsp; &nbsp; Contacto: {}</p>"#,
        esc(&q.number),
        esc(&q.number),
        format_date(q.date),
        esc(&q.company),
        esc(&q.rnc),
        esc(&q.subtitle),
        if q.client.is_empty() {
            "____________________________________".into()
        } else {
            esc(&q.client)
        },
        if q.contact.is_empty() {
            "__________________________".into()
        } else {
            esc(&q.contact)
        }
    );
    s.push_str("<h2>01 &nbsp; | &nbsp; EQUIPOS Y PERIFÉRICOS</h2><p>Equipo incluido en la presente propuesta.</p>");
    let mut rows = String::new();
    for i in &q.equipment {
        let mut desc = esc(&i.description);
        if i.option != "both" {
            desc.push_str(&format!(
                " · {}",
                esc(if i.option == "a" {
                    &q.option_a
                } else {
                    &q.option_b
                })
            ));
        }
        if i.tax != Decimal::from(18) {
            desc.push_str(&format!(" <small>(ITBIS {}%)</small>", i.tax));
        }
        rows.push_str(&row(&[
            i.quantity.normalize().to_string(),
            desc,
            money(i.price),
            money(line(i).2),
        ]));
    }
    s.push_str(&table(&["Cant.","Descripción","Precio antes de ITBIS","Importe con ITBIS"],rows,"<colgroup><col style='width:8%'><col style='width:49%'><col style='width:22%'><col style='width:21%'></colgroup>"));
    s.push_str("<h2>02 &nbsp; | &nbsp; LICENCIAMIENTO E INSTALACIÓN</h2>");
    rows = String::new();
    for i in &q.services {
        let option = match i.option.as_str() {
            "a" => format!("Opción A · {}", q.option_a),
            "b" => format!("Opción B · {}", q.option_b),
            _ => "Ambas modalidades".into(),
        };
        let mut desc = esc(&i.description);
        if i.quantity != Decimal::ONE {
            desc.push_str(&format!(" · Cant. {}", i.quantity.normalize()));
        }
        if i.tax != Decimal::ZERO {
            desc.push_str(&format!(" · ITBIS {}%", i.tax));
        }
        rows.push_str(&row(&[esc(&option), desc, money(line(i).2)]));
    }
    s.push_str(&table(&["Modalidad","Descripción","Importe"],rows,"<colgroup><col style='width:24%'><col style='width:56%'><col style='width:20%'></colgroup>"));
    s.push_str("<h2>03 &nbsp; | &nbsp; SERVICIO RECURRENTE</h2>");
    rows = String::new();
    for i in &q.recurring {
        let mut desc = esc(&i.description);
        if i.option != "both" {
            desc.push_str(&format!(
                " · modalidad {}",
                esc(if i.option == "a" {
                    &q.option_a
                } else {
                    &q.option_b
                })
            ));
        }
        if i.quantity != Decimal::ONE {
            desc.push_str(&format!(" · Cant. {}", i.quantity.normalize()));
        }
        if i.tax != Decimal::ZERO {
            desc.push_str(&format!(" · ITBIS {}%", i.tax));
        }
        rows.push_str(&row(&[desc, esc(&i.period), money(line(i).2)]));
    }
    s.push_str(&table(&["Concepto","Periodicidad","Importe"],rows,"<colgroup><col style='width:53%'><col style='width:27%'><col style='width:20%'></colgroup>"));
    s.push_str("<h2>RESUMEN ECONÓMICO REFERENCIAL</h2>");
    rows = String::new();
    let eq = [sum(&q.equipment, "a"), sum(&q.equipment, "b")];
    let sv = [sum(&q.services, "a"), sum(&q.services, "b")];
    for (label, values) in [
        ("Equipos, ITBIS incluido", eq),
        ("Instalación y producto", sv),
        ("Total inicial", [eq[0] + sv[0], eq[1] + sv[1]]),
    ] {
        rows.push_str(&row(&[label.into(), money(values[0]), money(values[1])]));
    }
    for period in ["Mensual", "Trimestral", "Anual", "Semanal"] {
        if !q.recurring.iter().any(|i| i.period == period) {
            continue;
        }
        let values: [String; 2] = ["a", "b"].map(|opt| {
            let items: Vec<_> = q
                .recurring
                .iter()
                .filter(|i| applies(i, opt) && i.period == period)
                .collect();
            if items.is_empty() {
                "No aplica".into()
            } else {
                money(items.iter().map(|i| line(i).2).sum())
            }
        });
        rows.push_str(&row(&[
            format!("Servicio {}", period.to_lowercase()),
            values[0].clone(),
            values[1].clone(),
        ]));
    }
    s.push_str(&table(&["Concepto",&format!("{} (A)",q.option_a),&format!("{} (B)",q.option_b)],rows,"<colgroup><col style='width:53%'><col style='width:24%'><col style='width:23%'></colgroup>"));
    s.push_str(&format!("<section class='notes'><h2>NOTAS</h2><div>{}</div></section><footer>{} · RNC {} · Cotización {}</footer></article><script src='/print.js'></script></body></html>",esc(&q.notes).replace('\n',"<br>"),esc(&q.company),esc(&q.rnc),esc(&q.number)));
    s.replace(
        "</head>",
        &format!("{}</head>", invoice::page_footer(q, "Cotización")),
    )
}
fn data_dir() -> PathBuf {
    if let Some(v) = env::var_os("GRIDEPOS_DATA_DIR") {
        return v.into();
    }
    #[cfg(target_os = "windows")]
    if let Some(v) = env::var_os("LOCALAPPDATA") {
        return PathBuf::from(v).join("GridePOS").join("Cotizaciones");
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/gridepos-cotizaciones")
}
fn random_id() -> String {
    let mut b = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|v| format!("{:02x}", v)).collect()
}
fn response(status: u16, mime: &str, body: Vec<u8>) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut r = Response::from_data(body).with_status_code(StatusCode(status));
    for (k,v) in [("Content-Type",mime),("Cache-Control","no-store"),("X-Content-Type-Options","nosniff"),("X-Frame-Options","DENY"),("Referrer-Policy","no-referrer"),("Content-Security-Policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; connect-src 'self'; frame-ancestors 'none'; object-src 'none'; base-uri 'none'; form-action 'self'")] {r.add_header(Header::from_bytes(k,v).unwrap());}
    r
}
fn read_body(req: &mut Request) -> AppResult<Vec<u8>> {
    if req.body_length().unwrap_or(0) > MAX_BODY as usize {
        return Err("El archivo supera los 8 MB.".into());
    }
    let mut b = Vec::new();
    req.as_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    if b.len() > MAX_BODY as usize {
        return Err("El archivo supera los 8 MB.".into());
    }
    Ok(b)
}
fn header<'a>(req: &'a Request, key: &'static str) -> Option<&'a str> {
    req.headers()
        .iter()
        .find(|h| h.field.equiv(key))
        .map(|h| h.value.as_str())
}
fn json_bytes(v: Value) -> Vec<u8> {
    serde_json::to_vec(&v).unwrap()
}
fn automatic_backup(db: &Connection, dir: &std::path::Path, reason: &str) -> AppResult<PathBuf> {
    let folder = dir.join("respaldos-automaticos");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let name = if reason == "diario" {
        format!("GridePOS-{}.sqlite3", Local::now().format("%Y-%m-%d"))
    } else {
        format!(
            "GridePOS-{}-{}-{}.sqlite3",
            reason,
            Local::now().format("%Y%m%d-%H%M%S"),
            random_id()
        )
    };
    let path = folder.join(name);
    if reason == "diario" && path.exists() {
        return Ok(path);
    }
    // SQLite creates a consistent snapshot, including committed WAL pages.
    db.execute("VACUUM main INTO ?1", [path.to_string_lossy().as_ref()])
        .map_err(|e| format!("No se pudo crear el respaldo automático: {e}"))?;
    Ok(path)
}
fn initialize_database(db: &mut Connection) -> AppResult<()> {
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS quotes(id TEXT PRIMARY KEY,payload TEXT NOT NULL,updated TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')));
        CREATE TABLE IF NOT EXISTS quote_revisions(quote_id TEXT NOT NULL, revision INTEGER NOT NULL, payload TEXT NOT NULL, saved_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')),PRIMARY KEY(quote_id,revision));").map_err(|e|e.to_string())?;
    let existing: Vec<(String, String, String)> = {
        let mut stmt = db
            .prepare("SELECT id,payload,updated FROM quotes")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for (id, payload, updated) in existing {
        let q: Quote = serde_json::from_str(&payload).map_err(|e| e.to_string())?;
        tx.execute("INSERT OR IGNORE INTO quote_revisions(quote_id,revision,payload,saved_at) VALUES(?1,?2,?3,?4)",params![id,q.revision,payload,updated]).map_err(|e|e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
fn handle(
    req: &mut Request,
    db: &mut Connection,
    token: &str,
    origin: &str,
    dir: &std::path::Path,
) -> AppResult<(u16, &'static str, Vec<u8>)> {
    let expected_host = origin.trim_start_matches("http://");
    if header(req, "Host") != Some(expected_host) {
        return Ok((
            403,
            "text/plain; charset=utf-8",
            b"Host no permitido".to_vec(),
        ));
    }
    if let Some(v) = header(req, "Origin") {
        if v != origin {
            return Ok((
                403,
                "text/plain; charset=utf-8",
                b"Origen no permitido".to_vec(),
            ));
        }
    }
    let path = req.url().split('?').next().unwrap_or("/").to_string();
    let method = req.method().as_str().to_string();
    if path.starts_with("/api/") && header(req, "X-Local-Token") != Some(token) {
        return Ok((401, "text/plain", b"Acceso local requerido".to_vec()));
    }
    let html = "text/html; charset=utf-8";
    let json_mime = "application/json; charset=utf-8";
    let static_asset = match path.as_str() {
        "/" => Some((html, INDEX.replace("__LOCAL_TOKEN__", token).into_bytes())),
        "/app.js" => Some(("text/javascript; charset=utf-8", JS.as_bytes().to_vec())),
        "/style.css" => Some(("text/css; charset=utf-8", CSS.as_bytes().to_vec())),
        "/logo.png" => Some(("image/png", LOGO.to_vec())),
        "/print.css" => Some((
            "text/css; charset=utf-8",
            include_bytes!("../static/print.css").to_vec(),
        )),
        "/print.js" => Some((
            "text/javascript; charset=utf-8",
            b"document.querySelector('#print').onclick=()=>window.print();".to_vec(),
        )),
        _ => None,
    };
    if let Some((mime, b)) = static_asset {
        return Ok((200, mime, b));
    }
    if method == "GET" && path == "/api/info" {
        return Ok((
            200,
            json_mime,
            json_bytes(
                json!({"data_dir":dir.display().to_string(),"backup_dir":dir.join("respaldos-automaticos").display().to_string(),"version":env!("CARGO_PKG_VERSION")}),
            ),
        ));
    }
    if method == "GET" && path == "/api/quotes" {
        let mut stmt = db
            .prepare("SELECT id,payload,updated FROM quotes ORDER BY updated DESC,id DESC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for r in rows {
            let (id, p, updated) = r.map_err(|e| e.to_string())?;
            let q: Quote = serde_json::from_str(&p).map_err(|e| e.to_string())?;
            result.push(json!({"id":id,"number":q.number,"client":q.client,"kind":q.kind,"updated":updated}));
        }
        return Ok((200, json_mime, json_bytes(json!(result))));
    }
    if method == "GET" && path == "/api/backup" {
        let mut stmt = db
            .prepare("SELECT id,payload FROM quotes ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for r in rows {
            let (id, p) = r.map_err(|e| e.to_string())?;
            result.push(json!({"id":id,"quote":serde_json::from_str::<Quote>(&p).map_err(|e|e.to_string())?}));
        }
        let mut stmt=db.prepare("SELECT quote_id,revision,payload,saved_at FROM quote_revisions ORDER BY quote_id,revision").map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut history = Vec::new();
        for row in rows {
            let (id, revision, payload, saved_at) = row.map_err(|e| e.to_string())?;
            history.push(json!({"id":id,"revision":revision,"quote":serde_json::from_str::<Quote>(&payload).map_err(|e|e.to_string())?,"saved_at":saved_at}));
        }
        return Ok((
            200,
            json_mime,
            json_bytes(
                json!({"format":"gridepos-backup","version":1,"created":Local::now().to_rfc3339(),"quotes":result,"history":history}),
            ),
        ));
    }
    if method == "POST" && path == "/api/restore" {
        #[derive(Deserialize)]
        struct Entry {
            id: String,
            quote: Quote,
        }
        #[derive(Deserialize)]
        struct HistoryEntry {
            id: String,
            revision: i64,
            quote: Quote,
            saved_at: String,
        }
        #[derive(Deserialize)]
        struct Backup {
            format: String,
            version: u32,
            quotes: Vec<Entry>,
            #[serde(default)]
            history: Vec<HistoryEntry>,
        }
        let v: Backup = serde_json::from_slice(&read_body(req)?)
            .map_err(|_| "El archivo no es un respaldo válido de GridePOS.".to_string())?;
        if v.format != "gridepos-backup"
            || v.version != 1
            || v.quotes.len() > 10000
            || v.history.len() > 100000
        {
            return Err("Formato de respaldo incompatible.".into());
        }
        for e in &v.quotes {
            e.quote.validate()?;
            if e.id.len() != 48 || !e.id.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("Identificador inválido en respaldo.".into());
            }
        }
        let ids: std::collections::HashSet<_> = v.quotes.iter().map(|e| e.id.as_str()).collect();
        if ids.len() != v.quotes.len() {
            return Err("El respaldo contiene identificadores duplicados.".into());
        }
        for e in &v.history {
            e.quote.validate()?;
            if !ids.contains(e.id.as_str())
                || e.revision != e.quote.revision
                || e.revision < 0
                || e.saved_at.len() > 100
            {
                return Err("Historial de versiones inválido en el respaldo.".into());
            }
        }
        automatic_backup(db, dir, "antes-de-restaurar")?;
        // Merge only missing IDs. Existing quotations are never overwritten by restore.
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let mut restored = 0;
        let mut added = std::collections::HashSet::new();
        for e in &v.quotes {
            let inserted = tx
                .execute(
                    "INSERT OR IGNORE INTO quotes(id,payload) VALUES (?1,?2)",
                    params![e.id, serde_json::to_string(&e.quote).unwrap()],
                )
                .map_err(|e| e.to_string())?;
            restored += inserted;
            if inserted > 0 {
                added.insert(e.id.as_str());
                tx.execute(
                    "INSERT INTO quote_revisions(quote_id,revision,payload) VALUES(?1,?2,?3)",
                    params![
                        e.id,
                        e.quote.revision,
                        serde_json::to_string(&e.quote).unwrap()
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        for e in &v.history {
            if added.contains(e.id.as_str()) {
                tx.execute("INSERT OR IGNORE INTO quote_revisions(quote_id,revision,payload,saved_at) VALUES(?1,?2,?3,?4)",params![e.id,e.revision,serde_json::to_string(&e.quote).unwrap(),e.saved_at]).map_err(|e|e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        return Ok((
            200,
            json_mime,
            json_bytes(json!({"restored":restored,"skipped":v.quotes.len()-restored})),
        ));
    }
    if method == "GET" && path.starts_with("/api/revisions/") {
        let parts: Vec<_> = path
            .trim_start_matches("/api/revisions/")
            .split('/')
            .collect();
        if parts.len() == 2 {
            let revision: i64 = parts[1].parse().map_err(|_| "Revisión inválida")?;
            let payload: Option<String> = db
                .query_row(
                    "SELECT payload FROM quote_revisions WHERE quote_id=?1 AND revision=?2",
                    params![parts[0], revision],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            return Ok(match payload {
                Some(p) => (200, json_mime, p.into_bytes()),
                None => (
                    404,
                    json_mime,
                    json_bytes(json!({"error":"Versión no encontrada"})),
                ),
            });
        }
        if parts.len() != 1 {
            return Err("Ruta de historial inválida".into());
        }
        let mut stmt=db.prepare("SELECT revision,saved_at FROM quote_revisions WHERE quote_id=?1 ORDER BY revision DESC").map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map([parts[0]], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for row in rows {
            let (revision, saved_at) = row.map_err(|e| e.to_string())?;
            result.push(json!({"revision":revision,"saved_at":saved_at}));
        }
        return Ok((200, json_mime, json_bytes(json!(result))));
    }
    let id = path.strip_prefix("/api/quotes/").map(str::to_owned);
    if method == "GET" {
        if let Some(id) = &id {
            let p: Option<String> = db
                .query_row("SELECT payload FROM quotes WHERE id=?1", [id], |r| r.get(0))
                .optional()
                .map_err(|e| e.to_string())?;
            return Ok(match p {
                Some(p) => (200, json_mime, p.into_bytes()),
                None => (
                    404,
                    json_mime,
                    json_bytes(json!({"error":"Cotización no encontrada"})),
                ),
            });
        }
    }
    if method == "POST" || method == "PUT" {
        if !["/api/calculate", "/api/print", "/api/quotes"].contains(&path.as_str()) && id.is_none()
        {
            return Ok((404, "text/plain", Vec::new()));
        }
        let mut q: Quote = serde_json::from_slice(&read_body(req)?)
            .map_err(|e| format!("Datos inválidos: {}", e))?;
        q.validate()?;
        if path == "/api/calculate" {
            return Ok((200, json_mime, json_bytes(totals(&q))));
        }
        if path == "/api/print" {
            return Ok((200, html, print_html(&q).into_bytes()));
        }
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let (id, revision) = if method == "PUT" {
            let id = id.ok_or("Falta la cotización a actualizar")?;
            let old: Option<String> = tx
                .query_row("SELECT payload FROM quotes WHERE id=?1", [&id], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(|e| e.to_string())?;
            let old = match old {
                Some(v) => v,
                None => {
                    return Ok((
                        404,
                        json_mime,
                        json_bytes(json!({"error":"Cotización no encontrada"})),
                    ))
                }
            };
            let previous: Quote = serde_json::from_str(&old).map_err(|e| e.to_string())?;
            if q.kind != previous.kind {
                return Err("Duplica o convierte el documento para cambiar de tipo.".into());
            }
            if q.revision != previous.revision {
                return Ok((
                    409,
                    json_mime,
                    json_bytes(
                        json!({"error":"Esta cotización cambió en otra ventana. Ábrela de nuevo antes de guardar."}),
                    ),
                ));
            }
            q.revision += 1;
            tx.execute("UPDATE quotes SET payload=?1,updated=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?2",params![serde_json::to_string(&q).unwrap(),id]).map_err(|e|e.to_string())?;
            (id, q.revision)
        } else {
            let id = random_id();
            q.revision = 1;
            tx.execute(
                "INSERT INTO quotes(id,payload) VALUES (?1,?2)",
                params![id, serde_json::to_string(&q).unwrap()],
            )
            .map_err(|e| e.to_string())?;
            (id, q.revision)
        };
        tx.execute(
            "INSERT INTO quote_revisions(quote_id,revision,payload) VALUES(?1,?2,?3)",
            params![id, revision, serde_json::to_string(&q).unwrap()],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        return Ok((
            200,
            json_mime,
            json_bytes(json!({"id":id,"revision":revision})),
        ));
    }
    Ok((
        404,
        "text/plain; charset=utf-8",
        "No encontrado".as_bytes().to_vec(),
    ))
}
fn run() -> AppResult<()> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear la carpeta de datos: {}", e))?;
    let db_path = dir.join("cotizaciones.sqlite3");
    let mut db = Connection::open(&db_path).map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    initialize_database(&mut db)?;
    if let Err(e) = automatic_backup(&db, &dir, "diario") {
        eprintln!("Aviso: {e}");
    }
    let server = Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
    let address = server
        .server_addr()
        .to_ip()
        .ok_or("No se pudo abrir la interfaz local")?;
    let origin = format!("http://{}", address);
    let token = random_id();
    println!("GridePOS Cotizaciones {}\n\nAplicación local: {}\nDatos: {}\n\nEsta ventana mantiene la aplicación abierta.\nCierra esta ventana para terminar. No requiere internet.",env!("CARGO_PKG_VERSION"),origin,dir.display());
    if env::var_os("GRIDEPOS_NO_BROWSER").is_none() {
        #[cfg(target_os = "windows")]
        let _ = Command::new("cmd")
            .args(["/C", "start", "", &origin])
            .spawn();
        #[cfg(target_os = "macos")]
        let _ = Command::new("open").arg(&origin).spawn();
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let _ = Command::new("xdg-open").arg(&origin).spawn();
    }
    for mut req in server.incoming_requests() {
        let (status, mime, body) = match handle(&mut req, &mut db, &token, &origin, &dir) {
            Ok(v) => v,
            Err(e) => (
                400,
                "application/json; charset=utf-8",
                json_bytes(json!({"error":e})),
            ),
        };
        let _ = req.respond(response(status, mime, body));
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!(
            "No se pudo iniciar GridePOS: {}\nPulsa Enter para cerrar.",
            e
        );
        let _ = std::io::stdin().read_line(&mut String::new());
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn quote() -> Quote {
        serde_json::from_value(json!({"number":"COT-TEST","date":"2026-10-14","company":"GRIDEPOS, S.R.L.","rnc":"1-33-79278-8","option_a":"Offline","option_b":"Online","equipment":[{"description":"Lector","quantity":1,"price":"1500.00","tax":18}],"services":[{"description":"Local","quantity":1,"price":10000,"tax":0,"option":"a"},{"description":"Externa","quantity":1,"price":4500,"tax":0,"option":"b"}],"recurring":[{"description":"Mantenimiento","quantity":1,"price":3000,"tax":0,"option":"b","period":"Mensual"}]})).unwrap()
    }
    #[test]
    fn approved_numbers() {
        let q = quote();
        q.validate().unwrap();
        let t = totals(&q);
        assert_eq!(t["a"]["initial"], "11770.00");
        assert_eq!(t["b"]["initial"], "6270.00");
        assert_eq!(t["b"]["periods"]["Mensual"], "3000");
    }
    #[test]
    fn round_half_up() {
        let mut i = quote().equipment[0].clone();
        i.price = Decimal::from_str("0.05").unwrap();
        i.tax = Decimal::from(10);
        assert_eq!(line(&i).1, Decimal::from_str("0.01").unwrap());
    }
    #[test]
    fn periods_do_not_mix() {
        let mut q = quote();
        let mut annual = q.recurring[0].clone();
        annual.period = "Anual".into();
        annual.price = Decimal::from(12000);
        q.recurring.push(annual);
        let t = totals(&q);
        assert_eq!(t["b"]["periods"]["Mensual"], "3000");
        assert_eq!(t["b"]["periods"]["Anual"], "12000");
        assert_eq!(t["b"]["initial"], "6270.00");
    }
    #[test]
    fn reject_invalid() {
        let mut q = quote();
        q.equipment[0].tax = Decimal::from(101);
        assert!(q.validate().is_err());
        q.equipment[0].tax = Decimal::ZERO;
        q.equipment[0].quantity = Decimal::ZERO;
        assert!(q.validate().is_err());
    }
    #[test]
    fn escape_print() {
        let mut q = quote();
        q.client = "<script>alert(1)</script>".into();
        let html = print_html(&q);
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("14 de octubre de 2026"));
    }
}
