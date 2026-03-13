# Dockerfile Multistage para Rust HFT (ARM64 / Graviton)
# Optimizando para baja latencia y mínima huella de memoria

# --- Etapa 1: Builder ---
FROM rust:slim-bookworm as builder

WORKDIR /usr/src/app

# Instalar dependencias de compilación
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copiar archivos de manifiesto para cachear dependencias
COPY Cargo.toml Cargo.lock ./
# Crear un proyecto dummy para compilar dependencias
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Ahora copiar el código real
COPY src ./src
# Compilar el binario final
RUN cargo build --release

# --- Etapa 2: Runtime (Imagen Final) ---
FROM debian:bookworm-slim

WORKDIR /app

# Instalar librerías necesarias para la ejecución
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

# Copiar el binario desde la etapa de construcción
COPY --from=builder /usr/src/app/target/release/trz_bot /app/trz_bot
COPY .env /app/.env

# Ejecutar el bot
CMD ["./trz_bot"]
