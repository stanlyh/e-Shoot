# e-Shoot

Herramienta de línea de comandos para enviar mensajes de WhatsApp masivos y programados usando la WhatsApp Business API (Meta Graph API).

## Casos de uso

- Campañas promocionales recurrentes (ofertas semanales, descuentos)
- Anuncios programados para una fecha y hora específica
- Mensajes personalizados por destinatario con sustitución de variables
- Seguimiento de ejecuciones con historial persistente

---

## Arquitectura

```
CLI (clap)
    ↓
Carga y validación de config.toml
    ↓
Base de datos SQLite (sqlx)
    ↓
SchedulerService (tokio-cron-scheduler)
    ↓
JobExecutor  →  renderiza plantillas  →  envía mensajes
    ↓
WhatsAppProvider  →  HTTP (reqwest)
    ↓
Meta Graph API
```

### Modos de operación

| Modo | Descripción |
|------|-------------|
| **Daemon** (`start`) | Corre en segundo plano ejecutando trabajos según su horario |
| **Ejecución inmediata** (`run-now`) | Ejecuta un trabajo puntualmente, ignorando el horario |
| **Gestión** (`check`, `list`, `history`, `show-config`) | Consulta y validación de configuración e historial |

---

## Estructura del proyecto

```
e-Shoot/
├── src/
│   ├── main.rs                  # Punto de entrada, enrutamiento de comandos CLI
│   ├── cli.rs                   # Definición de comandos y argumentos (clap)
│   ├── error.rs                 # Tipos de error (AppError, ConfigError)
│   ├── db.rs                    # Capa de base de datos SQLite
│   ├── executor.rs              # JobExecutor: renderizado y envío
│   ├── init.rs                  # Asistente interactivo para crear config.toml
│   ├── scheduler/
│   │   └── mod.rs               # SchedulerService: registro y arranque de trabajos
│   ├── config/
│   │   ├── mod.rs               # Carga y resolución de rutas de configuración
│   │   ├── schema.rs            # Estructuras: Config, JobDefinition, Template, etc.
│   │   └── validation.rs        # Validación de referencias entre trabajos y plantillas
│   └── messaging/
│       ├── client.rs            # HttpMessagingClient: HTTP con reintentos y auth
│       ├── template.rs          # Motor de plantillas (sustitución de {{1}}, {{2}}, …)
│       └── provider/
│           └── whatsapp.rs      # WhatsAppProvider: construcción de payloads para la API
├── migrations/
│   └── 0001_initial.sql         # Schema de SQLite
├── config.example.toml          # Ejemplo de configuración completo
└── Cargo.toml
```

---

## Flujo de datos

### Arranque del daemon (`e-shoot start`)

```
1. Cargar config.toml
2. Validar referencias (plantillas, grupos de destinatarios, crons)
3. Conectar a SQLite y correr migraciones
4. Crear HttpMessagingClient (token + reintentos)
5. Crear JobExecutor
6. SchedulerService registra cada trabajo:
   ├─ schedule (cron 6 campos)  →  register_cron_job()
   └─ schedule_once (ISO 8601)  →  register_once_job()  [solo si no se ejecutó antes]
7. Bloquear hasta Ctrl+C

Al dispararse un trabajo:
   ├─ Buscar plantilla y grupo de destinatarios
   ├─ Para cada número:
   │   ├─ Obtener variables del bloque [jobs.variables]
   │   ├─ Renderizar plantilla (sustituir {{1}}, {{2}}, …)
   │   └─ POST a /{phone_number_id}/messages  →  WhatsApp API
   └─ Registrar resultado en job_executions (enviados, fallidos, errores JSON)
```

### Ejecución inmediata (`e-shoot run-now <job_id>`)

Salta el planificador y ejecuta directamente el mismo `JobExecutor::execute()`.

---

## Base de datos

Ubicación predeterminada: `~/.local/share/e-shoot/e-shoot.db`

### `job_executions`

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `id` | INTEGER PK | Autoincremental |
| `job_id` | TEXT | Identificador del trabajo |
| `started_at` | TEXT | ISO 8601 UTC |
| `finished_at` | TEXT | ISO 8601 UTC |
| `sent` | INTEGER | Mensajes enviados correctamente |
| `failed` | INTEGER | Mensajes con error |
| `errors_json` | TEXT | JSON con `[{recipient, reason}]` |

### `fired_once_jobs`

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `job_id` | TEXT PK | Identificador del trabajo |
| `fired_at` | TEXT | ISO 8601 UTC |

Evita que trabajos `schedule_once` se re-ejecuten si el daemon se reinicia.

---

## Configuración (`config.toml`)

```toml
[api]
base_url            = "https://graph.facebook.com/v19.0"
token               = "${E_SHOOT_API_TOKEN}"   # o valor directo
phone_number_id     = "1234567890"
connect_timeout_secs = 5
request_timeout_secs = 30
max_retries         = 3
retry_backoff_secs  = 2

[logging]           # opcional
level  = "info"     # debug | trace | warn | error
format = "pretty"   # json
# file = "/var/log/e-shoot.log"

[[recipients]]
name    = "lista_promo"
numbers = ["+573001234567", "+573009876543"]

[[templates]]
name     = "promo_verano"
language = "es"

[[templates.components]]
type = "body"
[[templates.components.parameters]]
type  = "text"
value = "Hola {{1}}, tienes {{2}}% de descuento este {{3}}."

[[jobs]]
id          = "promo-lunes"
enabled     = true
description = "Envío cada lunes a las 9 AM"
schedule    = "0 0 9 * * MON"   # cron de 6 campos: seg min hora dom mes dow
template    = "promo_verano"
recipients  = "lista_promo"

[jobs.variables]
"+573001234567" = ["Eduardo", "20", "domingo"]
"+573009876543" = ["Ana",     "15", "sábado"]
```

Para trabajos de una sola vez, usa `schedule_once` en lugar de `schedule`:

```toml
schedule_once = "2026-06-01T10:00:00Z"
```

### Resolución de rutas

| Recurso | Orden de búsqueda |
|---------|-------------------|
| Config | Flag `--config` → env `E_SHOOT_CONFIG` → `~/.config/e-shoot/config.toml` → `./config.toml` |
| DB | Flag `--db` → env `E_SHOOT_DB` → `~/.local/share/e-shoot/e-shoot.db` |
| Token | `api.token` en config → env `E_SHOOT_API_TOKEN` (sobreescribe) |

---

## Comandos

```bash
# Generar config.toml de forma interactiva
e-shoot init [--output /ruta/config.toml]

# Validar la configuración
e-shoot check [--config /ruta/config.toml]

# Listar trabajos configurados
e-shoot list

# Ver historial de ejecuciones
e-shoot history [--job-id promo-lunes] [--limit 20]

# Mostrar la configuración resuelta (token redactado)
e-shoot show-config

# Arrancar el daemon
e-shoot start [--config /ruta/config.toml] [--db /ruta/db]

# Ejecutar un trabajo ahora mismo
e-shoot run-now <job_id>
```

---

## Stack tecnológico

| Capa | Tecnología |
|------|-----------|
| Runtime async | [Tokio](https://tokio.rs) |
| CLI | [clap](https://docs.rs/clap) (derive) |
| HTTP | [reqwest](https://docs.rs/reqwest) + rustls |
| Base de datos | [sqlx](https://docs.rs/sqlx) + SQLite |
| Planificador | [tokio-cron-scheduler](https://docs.rs/tokio-cron-scheduler) |
| Config | [toml](https://docs.rs/toml) + serde |
| Logging | [tracing](https://docs.rs/tracing) |
| Fechas | [chrono](https://docs.rs/chrono) |
| Errores | [thiserror](https://docs.rs/thiserror) |
| UI interactiva | [dialoguer](https://docs.rs/dialoguer) + [indicatif](https://docs.rs/indicatif) |

---

## Instalación

```bash
cargo build --release
# El binario queda en target/release/e-shoot
```

## Inicio rápido

```bash
# 1. Crear configuración
e-shoot init

# 2. Verificar que todo esté bien
e-shoot check

# 3. Probar un envío inmediato
e-shoot run-now <job_id>

# 4. Arrancar el daemon
e-shoot start
```
