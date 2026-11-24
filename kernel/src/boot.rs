// Boot information handed to the Kernel by Stage 2 of the bootloader.

// The type of region the kernel may use as RAM.
pub const USABLE: u32 = 1;

// A single region of the BIOS memory map
#[repr(C)]
pub struct Region {
    pub base: u64,
    pub length: u64,
    pub kind: u32,      // see kind_name()
    _attributes: u32    // ACPI 3.0 extras
}

impl Region {
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            USABLE => "usable",
            2 => "reserved",
            3 => "ACPI data",
            4 => "ACPI NVS",
            5 => "bad",
            _ => "unknown",
        }
    }
}

// Must have the same layout as `boot_info` in boot/stage2.asm
#[repr(C)]
pub struct BootInfo {
    pub boot_drive: u8,     // The BIOS drive number (0x80 is HD #1)
    region_count: u8,
    regions: [Region; 32],
}

impl BootInfo {
    // Regions filled in by bootloader stage 2.
    pub fn regions(&self) -> &[Region] {
        let count = (self.region_count as usize).min(self.regions.len());
        &self.regions[..count]
    }
}
