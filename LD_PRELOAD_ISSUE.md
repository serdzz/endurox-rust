# LD_PRELOAD libnstd.so Issue

## Problem

When running Rust applications that use Enduro/X UBF libraries, an error with undefined symbols occurs:

```
error while loading shared libraries: libubf.so: cannot open shared object file: No such file or directory
```

or

```
undefined symbol: ndrx_Bget_long
undefined symbol: ndrx_Badd_string
...
```

## Root Cause

The `libubf.so` library depends on symbols from `libnstd.so` (Enduro/X standard library), but the standard linker doesn't always correctly resolve these dependencies when dynamically loading libraries through FFI in Rust applications.

### Technical Detail

Enduro/X uses a multi-level library architecture:
- **libnstd.so** - base library with utility functions and common symbols
- **libubf.so** - UBF (Unified Buffer Format) library, depending on libnstd
- **libatmi.so** - ATMI (Application-to-Transaction Monitor Interface), also depending on libnstd

When loading `libubf.so` through Rust FFI, the dynamic linker doesn't always automatically load `libnstd.so`, leading to "undefined symbol" errors.

## Solution

Use `LD_PRELOAD` to force load `libnstd.so` **before** running the application:

```bash
export LD_PRELOAD=/opt/endurox/lib/libnstd.so
./your_rust_app
```

### For Docker

Add environment variable in `Dockerfile`:

```dockerfile
ENV LD_PRELOAD=/opt/endurox/lib/libnstd.so
```

Example from the project:
```dockerfile
ENV PATH="/opt/endurox/bin:${PATH}" \
    LD_LIBRARY_PATH="/opt/endurox/lib:${LD_LIBRARY_PATH}" \
    LD_PRELOAD=/opt/endurox/lib/libnstd.so \
    NDRX_HOME="/opt/endurox"
```

### For Shell Scripts

Add export in startup scripts:

```bash
#!/bin/bash

# Source environment
. /app/setenv.sh

# Preload libnstd.so to provide symbols for libubf.so
export LD_PRELOAD=/opt/endurox/lib/libnstd.so

# Run application
/app/bin/your_app
```

### For systemd Services

Add to unit file:

```ini
[Service]
Environment="LD_PRELOAD=/opt/endurox/lib/libnstd.so"
Environment="LD_LIBRARY_PATH=/opt/endurox/lib"
ExecStart=/app/bin/your_rust_app
```

## Alternative Solutions

### 1. Static Linking (not recommended)

You can try to statically link all Enduro/X libraries, but this:
- Increases binary size
- Complicates Enduro/X updates
- Can cause version conflicts

### 2. Explicit Loading via dlopen (complex)

In `build.rs` you can add explicit loading of `libnstd.so`:

```rust
// In build.rs
println!("cargo:rustc-link-arg=-Wl,--no-as-needed");
println!("cargo:rustc-link-lib=dylib=nstd");
println!("cargo:rustc-link-lib=dylib=ubf");
```

But this doesn't always work correctly with FFI.

### 3. Using rpath (works in some cases)

```rust
// In build.rs
println!("cargo:rustc-link-arg=-Wl,-rpath,/opt/endurox/lib");
```

However, `LD_PRELOAD` remains the most reliable solution.

## How to Verify

### 1. Check library dependencies

```bash
ldd /opt/endurox/lib/libubf.so
```

Should show:
```
libnstd.so => /opt/endurox/lib/libnstd.so (0x...)
libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x...)
...
```

### 2. Check loaded libraries at runtime

```bash
LD_PRELOAD=/opt/endurox/lib/libnstd.so ldd /app/bin/your_app
```

### 3. Check exported symbols

```bash
nm -D /opt/endurox/lib/libnstd.so | grep ndrx_
```

Should show all `ndrx_*` functions.

## Problem Manifestation

The problem manifests in the following cases:

### ✅ Works WITHOUT LD_PRELOAD:
- C/C++ applications compiled with correct `-l` flags
- Applications using only ATMI (without UBF)
- Statically linked applications

### ❌ Does NOT work WITHOUT LD_PRELOAD:
- Rust applications with FFI to UBF functions
- Go applications with CGo UBF calls
- Python applications with ctypes/cffi to libubf.so
- Any dynamically loaded modules

## In Our Project

### Where it's configured:

1. **Dockerfile** (line 40):
   ```dockerfile
   ENV LD_PRELOAD=/opt/endurox/lib/libnstd.so
   ```

2. **test_derive.sh** (line 7):
   ```bash
   export LD_PRELOAD=/opt/endurox/lib/libnstd.so
   ```

3. **GitLab CI** (.gitlab-ci.yml, line 50):
   ```yaml
   variables:
     LD_PRELOAD: /opt/endurox/lib/libnstd.so
   ```

### Components that need LD_PRELOAD:

- ✅ **ubf_test_client** - uses UBF directly
- ✅ **derive_macro_example** - uses UBF via derive macro
- ✅ **ubfsvr_rust** - UBF server
- ✅ **samplesvr_rust** - uses UBF for TRANSACTION service
- ✅ **Unit tests** - tests in endurox-sys/tests/

### Components that MAY work without LD_PRELOAD:

- ⚠️ **rest_gateway** - if not using UBF directly (depends on implementation)
- ⚠️ **xadmin** utilities - native C applications from Enduro/X

## Debugging

If the problem persists even with `LD_PRELOAD`:

### 1. Check library path:
```bash
ls -la /opt/endurox/lib/libnstd.so
```

### 2. Check access permissions:
```bash
# Should be readable for all
chmod 644 /opt/endurox/lib/libnstd.so
```

### 3. Check ldconfig cache:
```bash
ldconfig -p | grep nstd
```

If not found:
```bash
echo "/opt/endurox/lib" > /etc/ld.so.conf.d/endurox.conf
ldconfig
```

### 4. Use LD_DEBUG for diagnostics:
```bash
LD_DEBUG=libs LD_PRELOAD=/opt/endurox/lib/libnstd.so ./your_app 2>&1 | grep nstd
```

### 5. Check version conflicts:
```bash
strings /opt/endurox/lib/libnstd.so | grep "NDRX"
strings /opt/endurox/lib/libubf.so | grep "NDRX"
```

Versions should match.

## Conclusion

`LD_PRELOAD=/opt/endurox/lib/libnstd.so` is a **mandatory** setting for Rust applications using Enduro/X UBF.

### Checklist for new components:

- [ ] Add `LD_PRELOAD` to Dockerfile if used
- [ ] Add `export LD_PRELOAD` to shell startup scripts
- [ ] Add to CI/CD pipeline if there are UBF tests
- [ ] Document in component's README
- [ ] Verify it works in container and on bare metal

### Links:

- [Enduro/X Documentation](https://www.endurox.org/dokuwiki/)
- [Linux LD_PRELOAD](https://man7.org/linux/man-pages/man8/ld.so.8.html)
- [Dynamic Linker Tricks](https://www.akkadia.org/drepper/dsohowto.pdf)
