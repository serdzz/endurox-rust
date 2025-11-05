FROM --platform=linux/amd64 ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV NDRX_HOME=/opt/endurox

# Установка зависимостей
RUN apt-get update && apt-get install -y \
    curl \
    less\
    jq \
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

# Используем dpkg для установки, но перенаправляя в /opt/endurox
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

# Копирование workspace
WORKDIR /app
COPY Cargo.toml ./

# Копирование всех sub-crates
COPY endurox-sys ./endurox-sys
COPY endurox-derive ./endurox-derive
COPY samplesvr_rust ./samplesvr_rust
COPY rest_gateway ./rest_gateway
COPY ubfsvr_rust ./ubfsvr_rust
COPY ubf_test_client ./ubf_test_client

# Копирование и компиляция UBF field tables
COPY ubftab ./ubftab

RUN cd ubftab && \
    cp /opt/endurox/share/endurox/ubftab/* . && \
    mv Excompat Excompat.fd && \
    mv Exfields Exfields.fd && \
    mkfldhdr *.fd && \
    cp *.h /app/endurox-sys/src/

# Set FLDTBLDIR and FIELDTBLS environment variables
ENV FLDTBLDIR=/app/ubftab \
    FIELDTBLS=test

# Сборка всех server binaries
RUN cargo build --release && \
    mkdir -p /app/bin && \
    cp /app/target/release/samplesvr_rust /app/bin/ && \
    cp /app/target/release/rest_gateway /app/bin/ && \
    cp /app/target/release/ubfsvr_rust /app/bin/ && \
    cp /app/target/release/ubf_test_client /app/bin/

# Build derive macro example
RUN cd ubf_test_client && \
    cargo build --release --example derive_macro_example --features "ubf,derive" && \
    cp /app/target/release/examples/derive_macro_example /app/bin/

# Копирование конфигурационных файлов
COPY conf ./conf
COPY setenv.sh ./
COPY test_rest.sh ./
COPY test_derive.sh ./
RUN chmod +x test_rest.sh test_ubf.sh test_derive.sh

CMD ["/bin/bash"]
