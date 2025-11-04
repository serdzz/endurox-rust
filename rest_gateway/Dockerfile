FROM --platform=linux/amd64 ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV NDRX_HOME=/opt/endurox

# Установка зависимостей
RUN apt-get update && apt-get install -y \
    curl \
    libxml2 \
    build-essential \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Установка Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Установка Enduro/X из .deb пакета
WORKDIR /tmp
COPY endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb /tmp/
COPY endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb /tmp/

RUN dpkg-deb -x endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb /tmp/extracted && \
    mkdir -p /opt/endurox && \
    cp -r /tmp/extracted/usr/* /opt/endurox/ && \
    rm -rf /tmp/extracted /tmp/endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb && \
    dpkg-deb -x endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb /tmp/extracted && \
    cp -r /tmp/extracted/usr/* /opt/endurox/ && \
    rm -rf /tmp/extracted /tmp/endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb && \
    ldconfig /opt/endurox/lib

# Настройка переменных окружения
ENV PATH="/opt/endurox/bin:${PATH}" \
    LD_LIBRARY_PATH="/opt/endurox/lib:${LD_LIBRARY_PATH}" \
    PKG_CONFIG_PATH="/opt/endurox/lib/pkgconfig:${PKG_CONFIG_PATH}" \
    CPATH="/opt/endurox/include:${CPATH}" \
    NDRX_HOME="/opt/endurox"

# Копирование и сборка Rust проекта
WORKDIR /app
COPY Cargo.toml build.rs ./
COPY src ./src

RUN cargo build --release

# Копирование бинарника
RUN mkdir -p /opt/endurox/bin && \
    cp /app/target/release/rest_gateway /opt/endurox/bin/

CMD ["/opt/endurox/bin/rest_gateway"]
