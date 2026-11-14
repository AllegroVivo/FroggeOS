Set-Location $PSScriptRoot

$kernelSectors = 16     # must match KERNEL SECTORS in boot/stage2.asm

nasm -f bin boot/boot.asm -o boot.bin
if ($LASTEXITCODE -ne 0) { exit 1 }
nasm -f bin boot/stage2.asm -o stage2.bin
if ($LASTEXITCODE -ne 0) { exit 1 }

Push-Location kernel
cargo build
$cargoResult = $LASTEXITCODE
Pop-Location
if ($cargoResult -ne 0) { exit 1 }

$boot   = [System.IO.File]::ReadAllBytes("$PSScriptRoot\boot.bin")
$stage2 = [System.IO.File]::ReadAllBytes("$PSScriptRoot\stage2.bin")
$kernel = [System.IO.File]::ReadAllBytes("$PSScriptRoot\kernel\target\x86_64-unknown-none\debug\kernel")

if ($kernel.length -gt $kernelSectors * 512) {
    Write-Error "Kernel is $($kernel.length) bytes - too big for $kernelSectors sectors"
    exit 1
}

# Pad the kernel with zeros up to a whole number of sectors
$padded = New-Object byte[] ($kernelSectors * 512)
[Array]::Copy($kernel, $padded, $kernel.length)

[System.IO.File]::WriteAllBytes("$PSScriptRoot\disk.img", $boot + $stage2 + $padded)

qemu-system-x86_64 -fda disk.img
