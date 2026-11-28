# LilyPad: the FroggeOS filesystem

**Status (2026-11-28):** designed (roadmap ⚑ 5.1 and 5.2.1). `storage` and `mkimage` write it, the kernel mounts it at boot, and the shell's `ls`, `cat`, `cd` and `pwd` read it. ROADMAP.md tracks what's built.

**In one line:** a disk in fixed areas, and LilyPad, a small filesystem of our own in the spirit of TempleOS's RedSea: 512-byte blocks, a bitmap of free blocks, directories of 64-byte entries, and every file in one unbroken run of blocks.

---

## 1. The disk

`disk.img` is 8 MB: 16,384 sectors of 512 bytes, in four fixed areas.

| Sectors | Size | What |
|---|---|---|
| 0 | 512 bytes | Stage 1, the boot sector, ending in 55 AA |
| 1–4 | 2 KB | Stage 2 |
| 5–2047 | 1,021.5 KB | The kernel, which stage 2 loads by sector number. It can't pass 444 KB until 1.4 moves it, so the rest of this area stays empty. |
| 2048–16383 | 7 MB | LilyPad |

Stage 2 never reads LilyPad, and LilyPad doesn't record where the kernel is. Sector 2047, the last one in the kernel's area, is for tests: the selftest may write it, as long as it puts back what was there.

## 2. Blocks

- A block is 512 bytes, the same as a sector, so LilyPad's block n is disk sector 2048 + n.
- Every number is little-endian. Block numbers and counts take 32 bits, and sizes in bytes take 64.
- On the 8 MB disk, LilyPad has 14,336 blocks.

## 3. Block 0: the superblock

| Offset | Bytes | What |
|---|---|---|
| 0 | 8 | `LILYPAD!`, which the kernel checks before it trusts the disk |
| 8 | 4 | The version, 1 |
| 12 | 4 | The block size, 512 |
| 16 | 4 | How many blocks LilyPad has, this one included |
| 20 | 4 | The bitmap's first block, 1 |
| 24 | 4 | How many blocks the bitmap takes |
| 28 | 4 | Zero |
| 32 | 64 | The root directory's entry (§5), named `/` |
| 96 | 416 | Zero |

## 4. The bitmap

- It has one bit per block: bit b of the bitmap's byte i is block 8i + b, with bit 0 the lowest. 1 means used.
- The superblock, the bitmap, and every block of every file and directory are marked used.
- One bitmap block covers 4,096 blocks, so 14,336 blocks need 4 (blocks 1–4). The bits for blocks past the end are set, so they're never handed out.

## 5. Directory entries

Every file and directory is described by a 64-byte entry:

| Offset | Bytes | What |
|---|---|---|
| 0 | 32 | The name, padded with zero bytes. A first byte of 0 marks a free entry. |
| 32 | 4 | Flags: bit 0 is set for a directory, and the other bits are 0 |
| 36 | 4 | The first block, or 0 for an empty file |
| 40 | 8 | The size in bytes |
| 48 | 8 | When it last changed: the year (2 bytes), then the month, day, hour, minute and second (1 byte each), then a zero |
| 56 | 8 | Zero, for later |

Names:

- A name is 1 to 32 characters of printable ASCII, without spaces or `/`. The shell splits what's typed at spaces, so a name with one couldn't be typed.
- A name keeps its case, but looking one up ignores case: `cat readme.txt` finds `README.TXT`. So one directory can't hold two names that differ only in case.
- `.` and `..` aren't stored. The kernel works them out from the path.

## 6. Files and directories

- **A file is one unbroken run of blocks,** from its first block to first + ⌈size / 512⌉ − 1. The rest of its last block is zeros, and an empty file has no blocks.
- **A directory is a file of entries,** so its size is a multiple of 64. The root's entry lives in the superblock, and every other directory's lives in its parent.
- **A path** is names joined by `/`, starting from the root: `/docs/hello.txt`. In the shell, a path without the leading `/` starts from the current directory: in `/docs`, `hello.txt` is `/docs/hello.txt`.
- **A file that changes length** is written to a new run, and its old run is freed. So reading and writing whole files at a time (⚑ 5.6) suits LilyPad best.
- **A new run** is the first free run that's long enough (first fit), looking only from the block after the bitmap to LilyPad's end.
- **Times** are local time from the CMOS clock, as `date` shows them. `mkimage` takes them from the Windows files it packs.

## 7. Limits

- 32-bit block numbers reach 2 TB.
- A file needs a free run as long as itself. Free space can end up in runs too short for a big file even when there's enough room in total.

## 8. What uses it

- The `storage` crate: the format in Rust, with `cargo test` tests on Windows (5.2.2).
- `mkimage` (5.3): a Windows program that packs a folder into `disk.img`'s LilyPad, using the `storage` crate.
- The kernel: reading (5.4), then writing (5.5).
