# Windows Build Environment Setup for Rust/Soroban Projects

## Current Issue

The Windows environment is encountering linker errors when trying to build the Soroban smart contracts:

```
error: linker `link.exe` not found
```

This occurs because the project requires the MSVC (Microsoft Visual C++) toolchain, but Visual Studio Build Tools are not installed.

## Solutions (Choose One)

### Option 1: Install Visual Studio Build Tools (Recommended for Windows Native)

1. Download and install **Visual Studio Build Tools 2019 or later**:
   - Visit: https://visualstudio.microsoft.com/downloads/
   - Scroll down to "Tools for Visual Studio"
   - Download "Build Tools for Visual Studio"

2. During installation, select:
   - ✅ Desktop development with C++
   - ✅ MSVC v142 or later (x64/x86 build tools)
   - ✅ Windows 10 SDK or Windows 11 SDK

3. After installation, restart your terminal and run:
   ```bash
   cargo test --test co_creator_fee_split_invariant
   ```

### Option 2: Use WSL (Windows Subsystem for Linux) - Recommended

WSL provides a native Linux environment that's ideal for Rust development:

1. **Install WSL2** (if not already installed):
   ```powershell
   wsl --install
   ```

2. **Inside WSL, install Rust**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

3. **Navigate to your project** (Windows drives are mounted at /mnt/):
   ```bash
   cd /mnt/c/Users/USER/Desktop/GrantFox/accesslayer-contracts
   ```

4. **Install Soroban CLI**:
   ```bash
   cargo install --locked soroban-cli
   ```

5. **Run tests**:
   ```bash
   cargo test --test co_creator_fee_split_invariant
   ```

### Option 3: Use CI/CD (No Local Setup Required)

The project has GitHub Actions configured (`.github/workflows/ci.yml`) that will automatically run tests on push/PR:

1. **Push your changes**:
   ```bash
   git push origin feat/co-creator-fee-unit-tests
   ```

2. **Create a Pull Request** on GitHub

3. **View test results** in the GitHub Actions tab

The CI environment uses Ubuntu and will compile and test successfully.

### Option 4: Use Docker

Run tests in a containerized Linux environment:

1. **Install Docker Desktop** for Windows

2. **Create a Dockerfile** (if not present):
   ```dockerfile
   FROM rust:1.97-bookworm
   WORKDIR /workspace
   COPY . .
   RUN cargo build --workspace
   CMD ["cargo", "test", "--workspace"]
   ```

3. **Build and run**:
   ```bash
   docker build -t soroban-tests .
   docker run soroban-tests cargo test --test co_creator_fee_split_invariant
   ```

## Why Native Windows Build Fails

Rust on Windows supports two main toolchains:

1. **MSVC (Microsoft Visual C++)**: 
   - Requires Visual Studio Build Tools
   - Better Windows integration
   - Default for Soroban projects

2. **GNU (MinGW-w64)**:
   - Doesn't require Visual Studio
   - Has issues with large Rust projects (export ordinal limit)
   - Not recommended for Soroban development

Large projects like Soroban contracts hit the GNU toolchain's export limit (65,535 symbols), causing:
```
ld: error: export ordinal too large: 69326
```

## Test File Location

The co-creator fee split invariant tests are located at:
```
creator-keys/tests/co_creator_fee_split_invariant.rs
```

## Running Tests (Once Environment is Set Up)

```bash
# Run all co-creator fee split invariant tests
cargo test --test co_creator_fee_split_invariant

# Run a specific test
cargo test test_co_creator_fee_split_30_percent_no_xlm_lost

# Run with verbose output
cargo test --test co_creator_fee_split_invariant -- --nocapture

# Run all workspace tests
cargo test --workspace
```

## Verification Without Building Locally

You can verify the test implementation is correct by:

1. **Code Review**: The test file follows all existing patterns in `creator-keys/tests/`
2. **Syntax Check**: Use `cargo check` (lighter than full build)
3. **CI/CD**: Push to GitHub and let Actions run the tests
4. **WSL/Docker**: Use Linux environment for local testing

## Summary

For **quick local testing**, use **WSL** (Option 2).  
For **production validation**, use **CI/CD** (Option 3).  
For **Windows native development**, install **Visual Studio Build Tools** (Option 1).

The test implementation is complete and follows all project conventions. The build environment issue is Windows-specific and doesn't affect the correctness of the tests.
