use super::*;

// Hex escapes keep user-controlled values inside a CSS string and cannot close <style>.
fn css_string(s: &str) -> String {
    format!(
        "\"{}\"",
        s.chars()
            .map(|c| format!("\\{:x} ", c as u32))
            .collect::<String>()
    )
}
pub(super) fn page_footer(q: &Quote, label: &str) -> String {
    let content = css_string(&format!(
        "{} · RNC {} · {} {}",
        q.company, q.rnc, label, q.number
    ));
    format!("<style>@page {{ @bottom-center {{ content: {}; font-family: 'Times New Roman',Times,serif; font-size:8pt; color:#64748b; }} }}</style>",content)
}
pub(super) fn print_invoice(q: &Quote) -> String {
    let opt = &q.invoice_option;
    let mut s = format!(
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{}</title><link rel="stylesheet" href="/print.css">{}</head><body><nav class="print-tools"><button id="print">Imprimir / guardar PDF</button><span>Selecciona A4, escala 100% y desactiva los encabezados y pies del navegador.</span></nav><article class="invoice"><header><img src="/logo.png" alt="GridePOS"><div><h1>FACTURA COMERCIAL</h1><b>No. {}</b><p>Fecha: {}</p>{}</div></header><p class="company"><b>{} &nbsp; | &nbsp; RNC: {}</b></p><p class="subtitle">{}</p><h2>DATOS DEL CLIENTE</h2><p><b>Cliente / empresa:</b> {}<br><b>RNC / cédula:</b> {}<br><b>Contacto:</b> {}<br><b>Dirección:</b> {}</p>"#,
        esc(&q.number),
        page_footer(q, "Factura"),
        esc(&q.number),
        format_date(q.date),
        q.due_date
            .map(|d| format!("<p>Vencimiento: {}</p>", format_date(d)))
            .unwrap_or_default(),
        esc(&q.company),
        esc(&q.rnc),
        esc(&q.subtitle),
        esc(&q.client),
        esc(&q.client_rnc),
        esc(&q.contact),
        esc(&q.client_address)
    );
    if !q.source_number.is_empty() {
        s.push_str(&format!(
            "<p><b>Cotización de origen:</b> {}</p>",
            esc(&q.source_number)
        ));
    }
    if !q.fiscal_reference.trim().is_empty() {
        s.push_str(&format!(
            "<p><b>Referencia fiscal informada por el usuario:</b> {}</p>",
            esc(&q.fiscal_reference)
        ));
    }
    s.push_str(&format!(
        "<p><b>Modalidad facturada:</b> {}</p><h2>DETALLE DE LA FACTURA</h2>",
        esc(if opt == "a" { &q.option_a } else { &q.option_b })
    ));
    let mut rows = String::new();
    let mut base = Decimal::ZERO;
    let mut tax = Decimal::ZERO;
    for i in q
        .equipment
        .iter()
        .chain(q.services.iter())
        .filter(|i| applies(i, opt))
    {
        let (b, t, total) = line(i);
        base += b;
        tax += t;
        rows.push_str(&row(&[
            i.quantity.normalize().to_string(),
            esc(&i.description),
            money(i.price),
            format!("{}%<br>{}", i.tax.normalize(), money(t)),
            money(total),
        ]));
    }
    s.push_str(&table(&["Cant.","Descripción","Precio unitario","ITBIS","Importe"],rows,"<colgroup><col style='width:7%'><col style='width:42%'><col style='width:17%'><col style='width:16%'><col style='width:18%'></colgroup>"));
    s.push_str(&format!("<section class='invoice-totals'><div><span>Subtotal antes de ITBIS</span><b>{}</b></div><div><span>ITBIS</span><b>{}</b></div><div class='grand-total'><span>TOTAL FACTURADO</span><b>{}</b></div></section>",money(base),money(tax),money(base+tax)));
    let recurring: Vec<_> = q.recurring.iter().filter(|i| applies(i, opt)).collect();
    if !recurring.is_empty() {
        s.push_str("<h2>SERVICIOS RECURRENTES · INFORMATIVO</h2><p>Los importes siguientes no están incluidos en el total de esta factura. Para cobrar un período, añádelo como concepto en el detalle de la factura.</p>");
        let mut rows = String::new();
        for i in recurring {
            rows.push_str(&row(&[
                esc(&i.description),
                esc(&i.period),
                money(line(i).2),
            ]));
        }
        s.push_str(&table(
            &["Concepto", "Periodicidad", "Importe por período"],
            rows,
            "",
        ));
    }
    if !q.payment_terms.trim().is_empty() {
        s.push_str(&format!(
            "<h2>CONDICIONES DE PAGO</h2><p>{}</p>",
            esc(&q.payment_terms).replace('\n', "<br>")
        ));
    }
    s.push_str(&format!("<section class='notes'><h2>NOTAS</h2><div>{}</div><p class='fiscal-note'>Documento comercial generado localmente. La aplicación no emite ni valida NCF o e-CF autorizados por la DGII. Una referencia fiscal ingresada manualmente no implica validación fiscal.</p></section><footer>{} · RNC {} · Factura {}</footer></article><script src='/print.js'></script></body></html>",esc(&q.notes).replace('\n',"<br>"),esc(&q.company),esc(&q.rnc),esc(&q.number)));
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invoice_uses_selected_option_and_excludes_recurring() {
        let mut q:Quote=serde_json::from_value(json!({"kind":"invoice","invoice_option":"b","number":"FAC-001","date":"2026-11-12","company":"GRIDEPOS","rnc":"test","option_a":"Offline","option_b":"Online","equipment":[{"description":"Equipo","price":100,"quantity":2,"tax":18}],"services":[{"description":"Servicio Offline excluido","price":1000,"quantity":1,"tax":0,"option":"a"},{"description":"Instalación","price":50,"quantity":1,"tax":0,"option":"b"}],"recurring":[{"description":"Mensual","price":3000,"quantity":1,"tax":0,"option":"b"}]})).unwrap();
        q.validate().unwrap();
        let html = print_invoice(&q);
        assert!(html.contains("FACTURA COMERCIAL"));
        assert!(html.contains("RD$ 286.00"));
        assert!(!html.contains("Servicio Offline excluido"));
        assert!(html.contains("INFORMATIVO"));
        assert!(html.contains("no emite ni valida NCF"));
        q.fiscal_reference = "</style><script>alert(1)</script>".into();
        q.company = q.fiscal_reference.clone();
        let html = print_invoice(&q);
        assert!(!html.contains("</style><script>"));
        assert!(html.contains("&lt;/style&gt;"));
    }
}
