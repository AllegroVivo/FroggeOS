Set-Location $PSScriptRoot

$kernelSectors = 512     # must match KERNEL SECTORS in boot/stage2.asm

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

# Lay everything out on a blank 1.44MB Floppy image. Using the appropriate floppy
# size makes QEMU use the standard geometry.
$image = New-Object byte[] 1474560
[Array]::Copy($boot,   0, $image, 0,        $boot.Length)   # sector 0
[Array]::Copy($stage2, 0, $image, 512,      $stage2.Length) # sectors 1-4
[Array]::Copy($kernel, 0, $image, 5 * 512,  $kernel.Length) # sectors 5+
[System.IO.File]::WriteAllBytes("$PSScriptRoot\disk.img", $image)

qemu-system-x86_64 -drive file=disk.img,format=raw,if=floppy -rtc base=localtime -audiodev dsound,id=snd0 -machine pcspk-audiodev=snd0
