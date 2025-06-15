#!/usr/bin/env pwsh

# DataFlare Core Build Script
# This script builds the core DataFlare packages without OpenSSL dependencies

Write-Host "🚀 Building DataFlare Core Packages..." -ForegroundColor Green

# Set environment variables to avoid OpenSSL issues
$env:OPENSSL_NO_VENDOR = "1"
$env:OPENSSL_DIR = ""

# List of core packages to build
$corePackages = @(
    "dataflare-core",
    "dataflare-runtime", 
    "dataflare-processor",
    "dataflare-cli",
    "actix",
    "actix-broker",
    "actix_derive",
    "actix-cluster"
)

Write-Host "📦 Building packages: $($corePackages -join ', ')" -ForegroundColor Cyan

try {
    # Build core packages
    $buildArgs = @("build") + ($corePackages | ForEach-Object { "--package"; $_ })
    
    Write-Host "🔨 Running: rustup run nightly cargo $($buildArgs -join ' ')" -ForegroundColor Yellow
    
    & rustup run nightly cargo @buildArgs
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Core packages built successfully!" -ForegroundColor Green
        
        # Run tests for dataflare-core
        Write-Host "🧪 Running tests for dataflare-core..." -ForegroundColor Cyan
        & rustup run nightly cargo test --package dataflare-core --lib
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✅ All tests passed!" -ForegroundColor Green
        } else {
            Write-Host "❌ Some tests failed" -ForegroundColor Red
            exit 1
        }
    } else {
        Write-Host "❌ Build failed" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "❌ Build script failed: $_" -ForegroundColor Red
    exit 1
}

Write-Host "🎉 DataFlare core build completed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "📋 Summary:" -ForegroundColor Cyan
Write-Host "  - Built $($corePackages.Count) core packages" -ForegroundColor White
Write-Host "  - All tests passed" -ForegroundColor White
Write-Host "  - No OpenSSL dependencies required" -ForegroundColor White
Write-Host ""
Write-Host "🔧 Next steps:" -ForegroundColor Cyan
Write-Host "  - Run './dataflare --help' to see available commands" -ForegroundColor White
Write-Host "  - Check examples/ directory for usage examples" -ForegroundColor White
