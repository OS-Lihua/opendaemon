Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Usage {
    @"
usage:
  init.ps1 --component <tools...>
  init.ps1 --install <crates...>

examples:
  ./scripts/justfile/init.ps1 --component rust-analyzer clippy rustfmt
  ./scripts/justfile/init.ps1 --install prek cargo-nextest
"@
}

function Ensure-Command {
    param([Parameter(Mandatory)][string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "missing required command: $Name"
    }
}

function Get-ToolCommandCandidates {
    param([Parameter(Mandatory)][string]$Tool)

    switch ($Tool) {
        "clippy" { return @("cargo-clippy", "clippy-driver") }
        "rustfmt" { return @("rustfmt") }
        "rust-analyzer" { return @("rust-analyzer") }
        "wasm32-unknown-unknown" { return @() }
        default { return @($Tool) }
    }
}

function Get-BrewFormulaForTool {
    param([Parameter(Mandatory)][string]$Tool)

    switch ($Tool) {
        "clippy" { return "clippy" }
        "rust-analyzer" { return "rust-analyzer" }
        "rustfmt" { return "rust" }
        "trunk" { return "trunk" }
        "wasm-bindgen" { return "wasm-bindgen" }
        default { return $Tool }
    }
}

function Test-AnyCommand {
    param([Parameter(Mandatory)][string[]]$Names)

    foreach ($name in $Names) {
        if (Get-Command $name -ErrorAction SilentlyContinue) {
            return $true
        }
    }
    return $false
}

function Test-RustTarget {
    param([Parameter(Mandatory)][string]$Target)

    try {
        $targets = & rustc --print target-list 2>$null
    }
    catch {
        return $false
    }
    return ($targets -contains $Target)
}

function Ensure-Tool {
    param([Parameter(Mandatory)][string]$Tool)

    if ($Tool -eq "wasm32-unknown-unknown") {
        if (Test-RustTarget -Target $Tool) {
            return
        }
        [Console]::Error.WriteLine("error: missing Rust target: $Tool")
        [Console]::Error.WriteLine("install it with: rustup target add $Tool")
        [Console]::Error.WriteLine("note: Homebrew Rust does not manage additional Rust std targets reliably; rustup is the supported fallback for this target.")
        exit 1
    }

    $candidates = Get-ToolCommandCandidates -Tool $Tool
    if (Test-AnyCommand -Names $candidates) {
        return
    }

    [Console]::Error.WriteLine("error: missing required tool: $Tool")
    if (Get-Command brew -ErrorAction SilentlyContinue) {
        $formula = Get-BrewFormulaForTool -Tool $Tool
        [Console]::Error.WriteLine("install it with: brew install $formula")
    }
    elseif (Get-Command rustup -ErrorAction SilentlyContinue) {
        [Console]::Error.WriteLine("install it with: rustup component add $Tool")
    }
    exit 1
}

function Ensure-CargoTool {
    param([Parameter(Mandatory)][string]$Crate)

    $binary = $Crate
    if (Get-Command $binary -ErrorAction SilentlyContinue) {
        return
    }

    if (Get-Command cargo-binstall -ErrorAction SilentlyContinue) {
        & cargo binstall $Crate 2>$null
        if ($LASTEXITCODE -ne 0) {
            & cargo install --locked $Crate
            if ($LASTEXITCODE -ne 0) {
                exit $LASTEXITCODE
            }
        }
        return
    }

    & cargo install --locked $Crate
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

if ($args.Count -eq 0) {
    Usage
    exit 2
}

$i = 0
while ($i -lt $args.Count) {
    $arg = $args[$i]
    switch ($arg) {
        { $_ -in @("-h", "--help") } {
            Usage
            exit 0
        }
        "--component" {
            $i++
            $tools = @()
            while ($i -lt $args.Count -and -not $args[$i].StartsWith("--")) {
                $tools += $args[$i]
                $i++
            }
            if ($tools.Count -eq 0) {
                throw "--component requires at least 1 value"
            }
            foreach ($tool in $tools) {
                Ensure-Tool -Tool $tool
            }
        }
        "--install" {
            Ensure-Command cargo
            $i++
            $crates = @()
            while ($i -lt $args.Count -and -not $args[$i].StartsWith("--")) {
                $crates += $args[$i]
                $i++
            }
            if ($crates.Count -eq 0) {
                throw "--install requires at least 1 value"
            }
            foreach ($crate in $crates) {
                Ensure-CargoTool -Crate $crate
            }
        }
        default {
            throw "unknown argument: $arg"
        }
    }
}
