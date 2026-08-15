//! Mide la memoria residente del proceso al ejecutar un request cuyo body es
//! mucho más grande que el límite de retención, para comprobar que el cap de
//! streaming evita materializar el payload completo en RAM.
//!
//! Uso: cargo run -p midway-core --example mem_probe --release
//!
//! El servidor de prueba es un `TcpListener` que emite el body en chunks sin
//! retenerlo en memoria: usar un mock server que guarde el payload completo
//! contaminaría la medición, porque ese payload viviría en el mismo proceso
//! que estamos midiendo.
//!
//! Archivo de medición ad-hoc: no forma parte de la suite de tests.

use midway_core::domain::http::{HttpMethod, ResolvedBody, ResolvedRequest};
use midway_core::infra::http_reqwest::execute_request;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Lee la memoria residente (VmRSS) del proceso actual, en KB.
fn rss_kb() -> u64 {
    let status =
        std::fs::read_to_string("/proc/self/status").expect("no pude leer /proc/self/status");
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

const PAYLOAD_MB: usize = 200;
const CHUNK_SIZE: usize = 64 * 1024;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("no pude abrir el listener");
    let addr = listener.local_addr().expect("sin dirección local");

    // Servidor mínimo: responde con `PAYLOAD_MB` de body emitido en chunks de
    // 64 KB reutilizando el mismo buffer, así el emisor nunca retiene el
    // payload completo y no distorsiona la medición del receptor.
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("sin conexión entrante");

        let mut discard = [0_u8; 1024];
        let _ = socket.read(&mut discard).await;

        let total = PAYLOAD_MB * 1024 * 1024;
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {total}\r\n\r\n"
        );
        socket
            .write_all(headers.as_bytes())
            .await
            .expect("no pude escribir headers");

        let chunk = vec![b'z'; CHUNK_SIZE];
        let mut sent = 0_usize;
        while sent < total {
            let take = CHUNK_SIZE.min(total - sent);
            if socket.write_all(&chunk[..take]).await.is_err() {
                // El cliente cortó al alcanzar su límite: es el comportamiento
                // esperado, no un error.
                break;
            }
            sent += take;
        }
    });

    let baseline = rss_kb();
    println!("RSS antes del request:   {baseline:>9} KB");

    let client = reqwest::Client::new();
    let response = execute_request(
        &client,
        ResolvedRequest {
            method: HttpMethod::GET,
            url: format!("http://{addr}/huge"),
            headers: Vec::new(),
            body: ResolvedBody::None,
            timeout_ms: 120_000,
        },
    )
    .await
    .expect("el request debería completarse");

    let after = rss_kb();
    let delta_mb = after.saturating_sub(baseline) as f64 / 1024.0;
    let retained_mb = response.body_text.len() as f64 / (1024.0 * 1024.0);

    println!("RSS después del request: {after:>9} KB");
    println!();
    println!("Body enviado por el servidor: {PAYLOAD_MB} MB");
    println!("Body retenido en memoria:     {retained_mb:.1} MB");
    println!("Marcado como truncado:        {}", response.truncated);
    println!(
        "Content-Length informado:     {}",
        response
            .total_size_bytes
            .map(|total| format!("{:.1} MB", total as f64 / (1024.0 * 1024.0)))
            .unwrap_or_else(|| "desconocido".to_string())
    );
    println!("Delta de RSS:                 {delta_mb:.1} MB");
}
