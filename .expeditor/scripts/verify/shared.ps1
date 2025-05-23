. $PSScriptRoot\..\shared.ps1

function Install-HabPkg([string[]]$idents) {
    $idents | ForEach-Object {
        $id = $_
        $installedPkgs=hab pkg list $id | Where-Object { $_.StartsWith($id)}

        if($installedPkgs){
            Write-Host "$id already installed"
        } else {
            hab pkg install $id
        }
    }
}

function Initialize-Environment {
    $env:HAB_LICENSE = "accept-no-persist"
    Install-Habitat

    Install-HabPkg @(
        "core/cacerts",
        "core/protobuf",
        "core/visual-build-tools-2022",
        "core/zeromq",
        "core/windows-11-sdk"
    )

    # Set up some path variables for ease of use later
    $cacertsDir     = & hab pkg path core/cacerts
    $protobufDir    = & hab pkg path core/protobuf
    $zeromqDir      = & hab pkg path core/zeromq

    # Set some required variables
    $env:LIBZMQ_PREFIX              = "$zeromqDir"
    $env:SSL_CERT_FILE              = "$cacertsDir\ssl\certs\cacert.pem"
    $env:LD_LIBRARY_PATH            = "$env:LIBZMQ_PREFIX\lib;$env:SODIUM_LIB_DIR"
    $env:PATH                       = New-PathString -StartingPath $env:PATH -Path "$protobufDir\bin;$zeromqDir\bin"

    $vsDir = & hab pkg path core/visual-build-tools-2022
    $winSdkDir = & hab pkg path core/windows-11-sdk
    $env:LIB = "$(Get-Content "$vsDir\LIB_DIRS");$(Get-Content "$winSdkDir\LIB_DIRS");$env:LIBZMQ_PREFIX\lib"
    $env:INCLUDE = "$(Get-Content "$vsDir\INCLUDE_DIRS");$(Get-Content "$winSdkDir\INCLUDE_DIRS")"
    $env:PATH = New-PathString -StartingPath $env:PATH -Path (Get-Content "$vsDir\PATH")
    $env:PATH = New-PathString -StartingPath $env:PATH -Path (Get-Content "$winSdkDir\PATH")
    $oldPath = $env:PATH
    Invoke-Expression "$(hab pkg env core/visual-build-tools-2022 | Out-String)"
    $env:PATH = $oldPath
}

function Get-NightlyToolchain {
    "$(Get-Content $PSScriptRoot\..\..\..\RUST_NIGHTLY_VERSION)-x86_64-pc-windows-msvc"
}
