# Linux Installation Guide (Without Docker)

This guide explains how to install and run the Enduro/X Rust integration on a Linux system without using Docker.

## Prerequisites

- Ubuntu 22.04 or compatible Linux distribution
- Root/sudo access for system-wide installation
- Internet connection for downloading packages

## Step 1: Install System Dependencies

```bash
# Update package lists
sudo apt-get update

# Install required packages
sudo apt-get install -y \
    curl \
    jq \
    libxml2 \
    build-essential \
    pkg-config \
    git
```

## Step 2: Install Rust

Install Rust using rustup (if not already installed):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Load Rust environment
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

## Step 3: Install Enduro/X

### Option A: From Debian Packages (Recommended)

Download and install the Enduro/X packages:

```bash
# Create temporary directory
mkdir -p /tmp/endurox-install
cd /tmp/endurox-install

# Download packages (adjust URLs to your package location)
# Example: Copy from your project or download from Enduro/X repository
curl -O https://your-package-repository/endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb
curl -O https://your-package-repository/endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb

# Install Enduro/X core
sudo dpkg -i endurox-8.0.10-2.ubuntu22_04_gnu_epoll.x86_64.deb

# Install Enduro/X Connect (optional, for REST gateway)
sudo dpkg -i endurox-connect-8.0.4-1.ubuntu22_04.x86_64.deb

# Clean up
cd ~
rm -rf /tmp/endurox-install
```

### Option B: Build from Source

Follow the [Enduro/X build instructions](https://www.endurox.org/dokuwiki/doku.php?id=documentation:building) if packages are not available.

## Step 4: Clone the Repository

```bash
# Choose installation directory
cd ~
git clone <your-repository-url> endurox-rust
cd endurox-rust
```

## Step 5: Set Up Enduro/X Environment

### Configure Environment Variables

Edit `setenv.sh` to match your installation:

```bash
# Open setenv.sh in your editor
nano setenv.sh
```

Make sure these paths are correct:

```bash
export NDRX_APPHOME="$(pwd)"
export NDRX_HOME=/usr/local  # Or wherever Enduro/X is installed
```

### Load Environment

```bash
source ./setenv.sh
```

**Important**: You need to source this file in every new terminal session, or add it to your `.bashrc`/`.zshrc`:

```bash
echo "source $(pwd)/setenv.sh" >> ~/.bashrc
```

## Step 6: Generate UBF Field Headers

```bash
cd ubftab

# Copy Enduro/X system field definitions
cp /usr/local/share/endurox/ubftab/* . 2>/dev/null || true

# Rename to avoid conflicts
mv Excompat Excompat.fd 2>/dev/null || true
mv Exfields Exfields.fd 2>/dev/null || true

# Generate C headers from field definitions
mkfldhdr *.fd

# Copy headers to endurox-sys
cp *.h ../endurox-sys/src/

cd ..
```

## Step 7: Build the Project

```bash
# Build in release mode
cargo build --release

# Copy binaries to bin directory
mkdir -p bin
cp target/release/samplesvr_rust bin/
cp target/release/ubfsvr_rust bin/
cp target/release/rest_gateway bin/
cp target/release/ubf_test_client bin/
```

### Optional: Build Examples

```bash
# Build UBF derive macro example
cd ubf_test_client
cargo build --release --example derive_macro_example --features "ubf,derive"
cp ../target/release/examples/derive_macro_example ../bin/
cd ..
```

## Step 8: Configure Enduro/X

The project includes a default configuration file at `conf/ndrxconfig.xml`. Review and adjust if needed:

```bash
nano conf/ndrxconfig.xml
```

Key configuration points:
- Server instances (samplesvr_rust, ubfsvr_rust)
- Log file locations
- Client applications (rest_gateway)

## Step 9: Start Enduro/X Application

### Initial Setup

```bash
# Create necessary directories (if not already created by setenv.sh)
mkdir -p log tmp conf ubftab views lib

# Initialize configuration
xadmin provision -d -vaddubf
```

### Start the Application

```bash
# Start all services
xadmin start -y

# Check status
xadmin psc

# Check process model
xadmin ppm
```

Expected output from `xadmin psc`:
```
Nd Service Name Routine Name Prog Name SRVID #SUCC #FAIL MAX      LAST     STAT
-- ------------ ------------ --------- ----- ----- ----- -------- -------- -----
1  ECHO         ECHO         samplesvr_rust  2     0     0 00:00:00 00:00:00 AVAIL
1  HELLO        HELLO        samplesvr_rust  2     0     0 00:00:00 00:00:00 AVAIL
1  UBFECHO      UBFECHO      ubfsvr_rust     4     0     0 00:00:00 00:00:00 AVAIL
1  UBFTEST      UBFTEST      ubfsvr_rust     4     0     0 00:00:00 00:00:00 AVAIL
```

## Step 10: Test the Installation

### Test REST Gateway

The REST gateway should start automatically. Test it:

```bash
# Health check
curl http://localhost:8080/

# Test HELLO service
curl -X POST http://localhost:8080/api/hello \
  -H "Content-Type: application/json" \
  -d '{"name":"World"}'
```

### Test UBF Services

```bash
# Run UBF test client
./bin/ubf_test_client

# Run derive macro example
./bin/derive_macro_example
```

### View Logs

```bash
# View all logs
tail -f log/ULOG.*

# View specific service logs
tail -f log/samplesvr_rust.log
tail -f log/ubfsvr_rust.log
tail -f log/rest_gateway.log
```

## Managing the Application

### Stop Services

```bash
# Stop all services
xadmin stop -y

# Stop specific service
xadmin stop -s samplesvr_rust -i 2
```

### Restart Services

```bash
# Restart all
xadmin stop -y && xadmin start -y

# Restart specific service
xadmin restart -s ubfsvr_rust -i 4
```

### Check Status

```bash
# Print service catalog
xadmin psc

# Print process model
xadmin ppm

# View queue statistics
xadmin mqlq

# Print configuration
xadmin pc
```

## Troubleshooting

### Services Don't Start

Check the logs:
```bash
tail -n 100 log/ULOG.*
tail -n 100 log/ndrxd.log
```

Common issues:
- Environment variables not set (run `source ./setenv.sh`)
- Binary permissions (run `chmod +x bin/*`)
- Port conflicts (REST gateway uses port 8080)

### UBF Field Errors

If you see errors about field IDs:

```bash
# Regenerate UBF headers
cd ubftab
mkfldhdr *.fd
cp *.h ../endurox-sys/src/
cd ..

# Rebuild
cargo clean
cargo build --release
```

### Library Not Found

If you get "library not found" errors:

```bash
# Check library paths
echo $LD_LIBRARY_PATH

# Reload environment
source ./setenv.sh

# On some systems, you may need to update ldconfig
sudo ldconfig
```

### REST Gateway Not Responding

Check if the process is running:
```bash
ps aux | grep rest_gateway
```

Check the log:
```bash
tail -f log/rest_gateway.log
```

Manually start if needed:
```bash
cd bin
./rest_gateway
```

## Development Workflow

### Making Changes

1. Edit source code
2. Rebuild:
   ```bash
   cargo build --release
   cp target/release/samplesvr_rust bin/
   ```
3. Restart service:
   ```bash
   xadmin restart -s samplesvr_rust -i 2
   ```

### Adding New Services

1. Add service to `conf/ndrxconfig.xml`
2. Advertise service in your server code
3. Rebuild and restart Enduro/X

### Adding UBF Fields

1. Edit `ubftab/test.fd`
2. Regenerate headers:
   ```bash
   cd ubftab && mkfldhdr test.fd
   ```
3. Rebuild:
   ```bash
   cargo clean && cargo build --release
   ```

## Running as a System Service

To run Enduro/X as a systemd service:

### Create Service File

```bash
sudo nano /etc/systemd/system/endurox-rust.service
```

Content:
```ini
[Unit]
Description=Enduro/X Rust Application
After=network.target

[Service]
Type=forking
User=your-username
WorkingDirectory=/home/your-username/endurox-rust
Environment="PATH=/home/your-username/endurox-rust/bin:/usr/local/bin:/usr/bin:/bin"
ExecStartPre=/bin/bash -c 'source /home/your-username/endurox-rust/setenv.sh'
ExecStart=/usr/local/bin/xadmin start -y
ExecStop=/usr/local/bin/xadmin stop -y
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### Enable and Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable endurox-rust

# Start service
sudo systemctl start endurox-rust

# Check status
sudo systemctl status endurox-rust
```

## Performance Tuning

### Increase Worker Instances

Edit `conf/ndrxconfig.xml`:

```xml
<server name="samplesvr_rust">
    <min>2</min>  <!-- Increase minimum instances -->
    <max>5</max>  <!-- Increase maximum instances -->
    <srvid>2</srvid>
    <sysopt>-e ${NDRX_APPHOME}/log/samplesvr_rust.log -r</sysopt>
</server>
```

Apply changes:
```bash
xadmin reload
```

### Adjust Message Queue Sizes

Edit `setenv.sh`:

```bash
export NDRX_MSGSIZEMAX=131072  # Increase max message size
export NDRX_MSGMAX=500         # Increase queue depth
```

Restart Enduro/X for changes to take effect.

## Security Considerations

### File Permissions

```bash
# Restrict access to configuration
chmod 600 conf/*.xml
chmod 600 setenv.sh

# Ensure binaries are executable
chmod 755 bin/*
```

### Network Access

If exposing REST gateway externally:
- Use a reverse proxy (nginx, Apache)
- Enable TLS/SSL
- Implement authentication/authorization
- Configure firewall rules

## Additional Resources

- [Enduro/X Documentation](https://www.endurox.org/dokuwiki/)
- [Project README](README.md)
- [UBF Fields Guide](UBF_FIELDS_GUIDE.md)
- [UBF Struct Guide](UBF_STRUCT_GUIDE.md)
- [Multiple FD Files Guide](MULTIPLE_FD_FILES.md)

## Getting Help

If you encounter issues:

1. Check the logs in `log/` directory
2. Review Enduro/X documentation
3. Verify environment variables with `env | grep NDRX`
4. Check service status with `xadmin psc` and `xadmin ppm`
5. Test individual services with the test client
