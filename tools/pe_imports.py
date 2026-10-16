"""List direct and delay-loaded DLL imports from a Windows PE without a toolchain."""

import json
import struct
import sys
from pathlib import Path


def read_c_string(data: bytes, offset: int) -> str:
    end = data.find(b"\0", offset)
    if end < 0:
        raise ValueError("unterminated PE import name")
    return data[offset:end].decode("ascii")


def imports(path: Path) -> dict[str, list[str]]:
    data = path.read_bytes()
    if data[:2] != b"MZ":
        raise ValueError("not a Windows executable")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe : pe + 4] != b"PE\0\0":
        raise ValueError("invalid PE header")
    count = struct.unpack_from("<H", data, pe + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    magic = struct.unpack_from("<H", data, optional)[0]
    if magic not in (0x10B, 0x20B):
        raise ValueError("unsupported PE optional header")
    directories = optional + (112 if magic == 0x20B else 96)
    section_table = optional + optional_size
    sections = [
        struct.unpack_from("<IIII", data, section_table + i * 40 + 8)
        for i in range(count)
    ]

    def raw_offset(rva: int) -> int:
        for virtual_size, address, raw_size, raw_start in sections:
            if address <= rva < address + max(virtual_size, raw_size):
                offset = raw_start + rva - address
                if offset < len(data):
                    return offset
        raise ValueError(f"unmapped PE RVA {rva:#x}")

    result = {}
    for label, directory_index, stride, name_field in (
        ("direct", 1, 20, 12),
        ("delay_loaded", 13, 32, 4),
    ):
        rva, size = struct.unpack_from("<II", data, directories + directory_index * 8)
        names = []
        if rva and size:
            start = raw_offset(rva)
            for offset in range(start, start + size, stride):
                descriptor = data[offset : offset + stride]
                if descriptor == bytes(stride):
                    break
                name_rva = struct.unpack_from("<I", descriptor, name_field)[0]
                names.append(read_c_string(data, raw_offset(name_rva)))
        result[label] = sorted(set(names), key=str.casefold)
    return result


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: pe_imports.py path-to-exe")
    print(json.dumps(imports(Path(sys.argv[1])), indent=2))
