; boot/stage2.asm - Stage 2: real mode setup, switch to 32-bit protected
; mode, set up paging, then switch to 64-bit long mode.

bits 16
org 0x7E00

KERNEL_ADDR     equ 0x10000             ; where the kernel gets loaded and run
KERNEL_SEG      equ KERNEL_ADDR >> 4    ; same address as a real-mode segment
KERNEL_LBA      equ 5                   ; boot sector + 4 stage-2 sectors come first
CHUNK           equ 64                  ; sectors per read: 32 KB
MAX_REGIONS     equ 32                  ; memory-map regions boot_info has room for

start2:
    mov [boot_info.boot_drive], dl    ; BIOS gives the boot drive in DL, save it.
    mov si, msg2
    call print_string16

    call check_lba
    call load_kernel
    call read_memory_map
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

; Ensure the BIOS can read the disk by sector number (LBA) with the int 13h exts.
check_lba:
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [boot_info.boot_drive]
    int 0x13
    jc .missing             ; carry set: no extensions
    cmp bx, 0xAA55
    jne .missing            ; BX not swapped, no extensions
    test cx, 1
    jz .missing             ; bit 0 clear, no packet reads
    ret
.missing:
    mov si, lba_msg
    jmp halt_with

; Read KERNEL_SECTORS sectors, starting at disk sector KERNEL_LBA, into
; memory at KERNEL_ADDR: CHUNK sectors at a time with int 13h AH=42h,
; which takes the sector number in a Disk Address Packet.
load_kernel:
    mov bp, KERNEL_SECTORS  ; bp = sectors left to read
.next_chunk:
    mov ax, bp
    cmp ax, CHUNK
    jbe .read               ; the last read may be shorter
    mov ax, CHUNK
.read:
    mov [dap.count], ax
    mov si, dap             ; DS:SI = the packet
    mov dl, [boot_info.boot_drive]
    mov ah, 0x42            ; extended read
    int 0x13
    jc disk_error

    mov ax, [dap.count]
    add [dap.lba], ax                           ; the next sector to read
    add word [dap.segment], CHUNK * 512 / 16    ; 32 KB further on
    sub bp, ax
    jnz .next_chunk
    ret
    
; Request the memory map from the BIOS. One region pushed into boot_info 
; per call. Region size is 24 bytes: base, length, kind, and attributes.
read_memory_map:
    xor ebx, ebx                ; 0 = start with the first region
    mov di, boot_info.regions  ; ES:DI = wehere the region goes
.next:
    mov dword [di + 20], 1      ; attributes "valid"
    mov eax, 0xE820
    mov edx, 0x534D4150         ; BIOS checks for and echoes the 'SMAP'
    mov ecx, 24                 ; room for a single region
    int 0x15
    jc .done                    ; carry: no more regions or no E820
    cmp eax, 0x534D4150
    jne .done
    add di, 24
    inc byte [boot_info.region_count]
    cmp byte [boot_info.region_count], MAX_REGIONS
    je .done                    ; boot info is full
    test ebx, ebx
    jnz .next                   ; ebx is 0 after last region
.done:
    ret

disk_error:
    mov si, err_msg
halt_with:                      ; prints the message in SI then stops
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
    mov rdi, boot_info
    mov rax, KERNEL_ADDR
    call rax                ; Into the Rust kernel - it never returns

err_msg db "Kernel read error!", 0
lba_msg db "No LBA disk reads!", 0

align 8, db 0
boot_info:
.boot_drive:    db 0
.region_count:  db 0
                times 6 db 0
.regions:       times MAX_REGIONS * 24 db 0


; The Disk Address Packet that int 13h AH=42h reads from.
dap:
    db 0x10                 ; size of the packet
    db 0                    ; always 0
.count:     dw 0            ; sectors to read
.offset:    dw 0            ; where in memory: offset
.segment:   dw KERNEL_SEG   ; ... and segment
.lba:       dq KERNEL_LBA   ; the first sector to read

msg2 db "Stage 2 loaded!", 0
msg3 db "Protected mode reached!", 0
msg4 db "Long mode reached!", 0

times 2048 - ($ - $$) db 0
