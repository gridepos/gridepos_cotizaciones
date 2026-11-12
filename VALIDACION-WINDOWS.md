# Validación en Windows · 12 de noviembre de 2026

Se recuperó la implementación original del commit `5c536dd2658f8c02b9f54a61d03e4be2877faaa5` y se amplió conservando el formato y logo incluidos en ese repositorio.

## Resultados

- `cargo test --locked --offline`: seis pruebas unitarias y una integración aprobadas en Windows x64. La integración comprueba guardado, rechazo de revisiones desactualizadas, historial de versiones, respaldo, importación sin sobrescribir identificadores existentes y reapertura tras reiniciar el proceso.
- `cargo build --release --locked --offline`: ejecutable de producción compilado de forma nativa.
- `node --check static/app.js` y formato de Rust verificados.
- Instalador probado en una carpeta aislada y después ejecutado para el usuario. Accesos directos de escritorio e Inicio apuntan al ejecutable instalado. Los documentos se guardan fuera de la instalación.
- Prueba del ejecutable instalado con `GRIDEPOS_DATA_DIR` aislado: dos documentos y cuatro revisiones importados; al repetir la importación se conservaron los dos existentes; reapertura tras reinicio correcta. Tres copias SQLite automáticas pasaron `PRAGMA integrity_check`.
- Flujo en Chrome sobre el ejecutable Windows: edición y guardado de cotización, segunda revisión, recuperación de primera revisión y guardado como tercera, factura derivada con referencia al documento original y descarga de respaldo JSON con dos documentos y cuatro revisiones.
- Importes de referencia comprobados: RD$11,770.00 Offline, RD$6,270.00 Online y RD$3,000.00 mensuales por separado. La factura Online presenta RD$6,270.00 y no incluye el mantenimiento mensual en el total facturado.
- Vista de impresión de cotización y factura inspeccionada con el logo y sus secciones. HTML de impresión de ambos documentos comprobado también sobre la copia instalada.

## Límite de la prueba de PDF

El botón abrió el flujo de impresión. La solicitud de acceso al diálogo nativo de Google Chrome caducó sin autorización, por lo que no se automatizó ni se verificó el guardado final del archivo PDF en Windows. La vista de impresión sí fue verificada. El usuario puede guardar desde ese diálogo con Guardar como PDF o Microsoft Print to PDF siguiendo la guía. No se verificó impresión física.

## Protección de datos

Todas las pruebas utilizaron carpetas separadas y documentos identificados con prefijo `QA-`, sin datos reales de clientes. Las bases de datos, respaldos, capturas y ejecutables de prueba están excluidos de Git. El README y el archivo de exclusiones iniciales del proyecto local se conservaron en `.local-inicial-20261112`.

SHA-256 del ejecutable probado e instalado:

`5410a77ba509d5d0195ff5c97ef6b803460577f60eababd8a63ac3537ff52a5d`

La validación funcional no constituye certificación ni validación fiscal ante la DGII.
