; boot/stage2.asm - Stage 2: real mode setup, switch to 32-bit protected
; mode, set up paging, then switch to 64-bit long mode.

bits 16
org 0x7E00

KERNEL_ADDR     equ 0x10000             ; where the kernel gets loaded and run
KERNEL_SEG      equ KERNEL_ADDR >> 4    ; same address as a real-mode segment
KERNEL_LBA      equ 5                   ; boot sector + 4 stage-2 sectors come first
KERNEL_SECTORS  equ 256                 ; 256 * 512 = 128KB; build.ps1 must match

start2:
    mov [boot_drive], dl    ; BIOS gives the boot drive in DL, save it.
    mov si, msg2
    call print_string16

    call load_kernel
    call enable_a20

    cli                     ; no more interrupts from here on - BIOS
                              ; interrupt handlers are about to become
                              ; meaningless anyway once we leave real mode
    lgdt [gdt_descriptor]

    mov eax, cr0
    or eax, 1               ; set the Protection Enable bit
    mov cr0, eax

    jmp CODE_SEG:protected_mode_start      ; far jump into 32-bit code;
                                              ; also flushes the CPU's
                                              ; instruction prefetch cache

; ---- 16-bit helpers (must stay before the bits 32 switch) ----

; Read KERNEL_SECTORS sectors, starting at disk sector KERNEL_LBA, into
; memory at KERNEL_ADDR. One sector at a time, because a floppy read can't
; cross a track boundary.
load_kernel:
    mov ax, KERNEL_SEG
    mov es, ax
    xor bx, bx              ; ES:BX = destination
    mov di, KERNEL_LBA      ; di = current sector number (LBA)
    mov bp, KERNEL_SECTORS  ; bp = sectors left to read
.next_sector:
    mov ax, di
    xor dx, dx
    mov cx, 18              ; A 1.44MB floppy has 18 sectors per track
    div cx                  ; ax = track number, dx = sector within track
    mov cl, dl
    inc cl                  ; BIOS sector numbers start at 1
    mov dh, al
    and dh, 1               ; head = track & 1  (2 heads)
    mov ch, al
    shr ch, 1               ; cylinder = track / 2
    mov dl, [boot_drive]
    mov ax, 0x0201          ; AH = 2 read sectors, AL = 1 sector
    int 0x13
    jc disk_error

    mov ax, es
    add ax, 0x20            ; 512 bytes = 0.20 paragraphs
    mov es, ax              ; move the destination by one sector
    inc di
    dec bp
    jnz .next_sector
    ret

disk_error:
    mov si, err_msg
    call print_string16
.halt:
    cli
    hlt
    jmp .halt

enable_a20:
    in al, 0x92
    or al, 2
    out 0x92, al
    ret

print_string16:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp print_string16
.done:
    ret

; ---- Global Descriptor Table ----

gdt_start:
    dq 0x0000000000000000           ; null descriptor - required by the CPU

gdt_code:
    dw 0xFFFF                       ; limit 0-15
    dw 0x0000                       ; base 0-15
    db 0x00                         ; base 16-23
    db 10011010b                    ; access: present, ring0, code, exec, read
    db 11001111b                    ; flags: 4K granularity, 32-bit + limit 16-19
    db 0x00                         ; base 24-31

gdt_data:
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 10010010b                    ; access: present, ring0, data, write
    db 11001111b
    db 0x00

gdt_code64:
    dw 0xFFFF                       ; limit - ignored in 64-bit mode
    dw 0x0000                       ; base - ignored in 64-bit mode
    db 0x00
    db 10011010b                    ; access: present, ring0, code, exec, read
    db 10100000b                    ; flags: G=1, L=1 (long mode), D=0
    db 0x00                         ; base 24-31

gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1      ; size of GDT minus 1, as the CPU wants
    dd gdt_start                    ; the linear address of the GDT

CODE_SEG   equ gdt_code - gdt_start
DATA_SEG   equ gdt_data - gdt_start
CODE64_SEG equ gdt_code64 - gdt_start

; ---- 32-bit protected mode code ----

bits 32

protected_mode_start:
    mov ax, DATA_SEG
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000                ; stack somewhere safe and unused in low RAM

    ; clear the screen - makes the mode switch visually obvious
    mov edi, 0xB8000
    mov ecx, 80 * 25
    mov ax, 0x0F20                  ; space char, white on black attribute
.clear:
    mov [edi], ax
    add edi, 2
    loop .clear

    ; print out the message directly to video memory - no BIOS involved
    mov esi, msg3
    mov edi, 0xB8000
    mov ah, 0x0F
.print32:
    lodsb
    or al, al
    jz .setup_paging
    mov [edi], al
    mov [edi + 1], ah
    add edi, 2
    jmp .print32

; ---- Set up minimal identity-mapped page tables for long mode ----
; PML4 at 0x1000, PDPT at 0x2000, PD at 0x3000 - all comfortably below
; out own code at 0x7C00 and clear of the IVT/BDA at 0x0-0x4FF.

.setup_paging:
    mov edi, 0x1000
    mov ecx, 0x3000 / 4         ; zero all three 4KB tables (12KB = 3072 dwords)
    xor eax, eax
    rep stosd

    mov dword [0x1000], 0x2003  ; PML4[0] -> PDPT (present, writable)
    mov dword [0x2000], 0x3003  ; PDPT[0] -> PD   (present, writable)

    mov edi, 0x3000
    mov eax, 0x83               ; present, writeable, 2MB page (PS bit)
    mov ecx, 512
.map_pd:
    mov [edi], eax
    add eax, 0x200000           ; next 2MB page
    add edi, 8
    loop .map_pd

    mov eax, 0x1000
    mov cr3, eax                ; point CR3 at the PML4

    mov eax, cr4
    or eax, 0x20                ; set PAE
    mov cr4, eax

    mov ecx, 0xC0000080         ; EFER MSR
    rdmsr
    or eax, 0x100               ; set LME (Long Mode Enable)
    wrmsr

    mov eax, cr0
    or eax, 0x80000000          ; set PG - paging on, long move active
    mov cr0, eax

    jmp CODE64_SEG:long_mode_start  ; far jump loads a 64-bit CS,
                                      ; fully entering long mode

; ---- 64-bit long mode code ----

bits 64
long_mode_start:
    mov ax, 0
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov rsp, 0x90000

    mov rdi, 0xB8000
    mov rcx, 80 * 25
    mov ax, 0x0F20
.clear64:
    mov [rdi], ax
    add rdi, 2
    loop .clear64

    mov rsi, msg4
    mov rdi, 0xB8000
    mov ah, 0x0F
.print64:
    lodsb
    or al, al
    jz .enter_kernel
    mov [rdi], al
    mov [rdi + 1], ah
    add rdi, 2
    jmp .print64
.enter_kernel:
    mov rax, KERNEL_ADDR
    call rax                ; Into the Rust kernel - it never returns

boot_drive db 0
err_msg db "Kernel read error!", 0

msg2 db "Stage 2 loaded!", 0
msg3 db "Protected mode reached!", 0
msg4 db "Long mode reached!", 0

times 2048 - ($ - $$) db 0
