; boot/boot.asm - Stage 1 bootloader (MBR boot sector)
;
; The BIOS loads this exact file (once assembled) into memory at
; address 0x7C00 and jumps to it. We're in 16-bit real mode.

bits 16
org 0x7C00

start:
    ; BIOS goesn't guarantee segment registers are zeroed on entry,
    ; so we don't set them up outselves before touching memory.
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00       ; stack grows downward from just below our code

    mov [boot_drive], dl    ; BIOS passes the boot drive number in DL at boot
                              ; save it - we need to read from the same disk.

    mov si, msg
    call print_string

    ; Load stage 2 off disk: 4 sectors, starting right after the boot
    ; sector, into memory right after 0x7E00
    mov ah, 0x02        ; BIOS function 0x02: read sectors into memory
    mov al, 4           ; number of sectors to read
    mov ch, 0           ; cylinder 0
    mov cl, 2           ; start at sector 2 (sector 1 is current)
    mov dh, 0           ; head 0
    mov dl, [boot_drive]
    mov bx, 0x7E00      ; ES:BX destination (ES is already 0)
    int 0x13
    jc disk_error

    jmp 0x7E00          ; hand off execution to stage 2

disk_error:
    mov si, err_msg
    call print_string

halt:
    cli                 ; disable interrupts
    hlt                 ; halt the CPU until the next interrupt
    jmp halt

print_string:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp print_string
.done:
    ret

boot_drive db 0
msg db "Booting FroggeOS...", 0
err_msg db "Disk read error!", 0

times 510 - ($ - $$) db 0   ; pad the rest of the 512-byte sector with zeros
dw 0xAA55                   ; boot signature - BIOS checks for this exact value
                              ; at byes 511-512, or it won't boot our OS.