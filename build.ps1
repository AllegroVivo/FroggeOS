param([switch]$SelfTest)

Set-Location $PSScriptRoot

Push-Location kernel
if ($SelfTest) {
    cargo build --features selftest
} else {
    cargo build
}
$cargoResult = $LASTEXITCODE
Pop-Location
if ($cargoResult -ne 0) { exit 1 }

$kernel = [System.IO.File]::ReadAllBytes("$PSScriptRoot\kernel\target\x86_64-unknown-none\debug\kernel")

# The kernel loads at 0x10000 and must end before the stack's guard page at 0x7F000.
if ($kernel.length -gt 0x7F000 - 0x10000) {
    Write-Error "Kernel is $($kernel.length) bytes - too big for the 444 KB below the stack"
    exit 1
}
$kernelSectors = [Math]::Ceiling($kernel.Length / 512)

nasm -f bin boot/boot.asm -o boot.bin
if ($LASTEXITCODE -ne 0) { exit 1 }
nasm -f bin -D KERNEL_SECTORS=$kernelSectors boot/stage2.asm -o stage2.bin
if ($LASTEXITCODE -ne 0) { exit 1 }

$boot   = [System.IO.File]::ReadAllBytes("$PSScriptRoot\boot.bin")
$stage2 = [System.IO.File]::ReadAllBytes("$PSScriptRoot\stage2.bin")

# Lay everything out on a blank 8MB HD image. (See FILESYSTEM.md)
$image = New-Object byte[] (8 * 1024 * 1024)
[Array]::Copy($boot,   0, $image, 0,        $boot.Length)   # sector 0
[Array]::Copy($stage2, 0, $image, 512,      $stage2.Length) # sectors 1-4
[Array]::Copy($kernel, 0, $image, 5 * 512,  $kernel.Length) # sectors 5+
[System.IO.File]::WriteAllBytes("$PSScriptRoot\disk.img", $image)

cargo run --quiet --manifest-path mkimage\Cargo.toml -- disk.img files
if ($LASTEXITCODE -ne 0) { exit 1 }

if ($SelfTest) {
    qemu-system-x86_64 -drive file=disk.img,format=raw,if=ide -rtc base=localtime -m 4G -display none -serial stdio -no-reboot -device isa-debug-exit,iobase=0xf4,iosize=0x04
    # QEMU exits with the value the kernel wrote times 2 plus 1.
    if ($LASTEXITCODE -ne 33) {
        Write-Error "selftest failed (QEMU exit code $LASTEXITCODE)"
        exit 1
    }
    "selftest passed"
    exit 0
} else {
    qemu-system-x86_64 -drive file=disk.img,format=raw,if=ide -rtc base=localtime -m 4G -audiodev dsound,id=snd0 -machine pcspk-audiodev=snd0 -serial stdio
}
