FROM --platform=linux/amd64 ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV NDRX_HOME=/opt/endurox

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    less\
    jq \
    libxml2 \
    build-essential \
    pkg-config \
    libaio1 \
    wget \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install Oracle Instant Client
RUN mkdir -p /opt/oracle && \
    cd /opt/oracle && \
    wget https://download.oracle.com/otn_software/linux/instantclient/2340000/instantclient-basic-linux.x64-23.4.0.24.05.zip && \
    unzip instantclient-basic-linux.x64-23.4.0.24.05.zip && \
    rm instantclient-basic-linux.x64-23.4.0.24.05.zip && \
    echo /opt/oracle/instantclient_23_4 > /etc/ld.so.conf.d/oracle-instantclient.conf && \
    ldconfig

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Enduro/X from .deb package
WORKDIR /tmp
COPY endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb /tmp/
COPY endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb /tmp/

# Use dpkg for installation, redirecting to /opt/endurox
RUN dpkg-deb -x endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb /tmp/extracted && \
    mkdir -p /opt/endurox && \
    cp -r /tmp/extracted/usr/* /opt/endurox/ && \
    rm -rf /tmp/extracted /tmp/endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb && \
    dpkg-deb -x endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb /tmp/extracted && \
    cp -r /tmp/extracted/usr/* /opt/endurox/ && \
    rm -rf /tmp/extracted /tmp/endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb && \
    ldconfig /opt/endurox/lib

# Configure environment variables
ENV PATH="/opt/endurox/bin:${PATH}" \
    LD_LIBRARY_PATH="/opt/endurox/lib:/opt/oracle/instantclient_23_4:${LD_LIBRARY_PATH}" \
    PKG_CONFIG_PATH="/opt/endurox/lib/pkgconfig:${PKG_CONFIG_PATH}" \
    CPATH="/opt/endurox/include:${CPATH}" \
    LD_PRELOAD=/opt/endurox/lib/libnstd.so \
    NDRX_HOME="/opt/endurox" \
    ORACLE_HOME=/opt/oracle/instantclient_23_4

# Copy workspace
WORKDIR /app
COPY Cargo.toml ./

# Copy all sub-crates
COPY endurox-sys ./endurox-sys
COPY endurox-derive ./endurox-derive
COPY samplesvr_rust ./samplesvr_rust
COPY rest_gateway ./rest_gateway
COPY ubfsvr_rust ./ubfsvr_rust
COPY ubf_test_client ./ubf_test_client
COPY oracle_txn_server ./oracle_txn_server

# Copy and compile UBF field tables
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

# Build all server binaries
RUN cargo build --release && \
    mkdir -p /app/bin && \
    cp /app/target/release/samplesvr_rust /app/bin/ && \
    cp /app/target/release/rest_gateway /app/bin/ && \
    cp /app/target/release/ubfsvr_rust /app/bin/ && \
    cp /app/target/release/ubf_test_client /app/bin/ && \
    cp /app/target/release/oracle_txn_server /app/bin/ || true

# Build derive macro example
RUN cd ubf_test_client && \
    cargo build --release --example derive_macro_example --features "ubf,derive" && \
    cp /app/target/release/examples/derive_macro_example /app/bin/

# Copy configuration files
COPY conf ./conf
COPY setenv.sh ./
COPY test_rest.sh ./
COPY test_derive.sh ./
RUN chmod +x test_rest.sh  test_derive.sh

CMD ["/bin/bash"]
