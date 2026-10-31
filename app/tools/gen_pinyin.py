# -*- coding: utf-8 -*-
"""生成 src-tauri/src/pinyin_data.rs 的汉字读音表。

只在开发期运行，需要 pypinyin。产物是一份纯数据的 Rust 源文件，
运行时不依赖 Python。

    python tools/gen_pinyin.py

表格式：每行「汉字 空格 读音1,读音2,...」，多音字全收（检索用，
用户打 hangkai 也要能命中「行楷」）。
"""

import io
import os

from pypinyin import Style, pinyin

FIRST = 0x4E00
LAST = 0xA000

OUT = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "src", "pinyin_data.rs")


def readings(ch: str) -> list[str]:
    got = pinyin(ch, style=Style.NORMAL, heteronym=True)
    if not got or not got[0]:
        return []
    out, seen = [], set()
    for raw in got[0]:
        word = raw.lower()
        if not word.isascii() or not word.isalpha():
            continue
        if word not in seen:
            seen.add(word)
            out.append(word)
    return out[:4]


def main() -> None:
    lines = []
    for cp in range(FIRST, LAST):
        ch = chr(cp)
        read = readings(ch)
        if read:
            lines.append("%s %s" % (ch, ",".join(read)))

    chunks = []
    step = 24
    for i in range(0, len(lines), step):
        body = "".join(line + "\\n" for line in lines[i : i + step])
        chunks.append('    "%s",\n' % body)

    header = (
        "//! 汉字读音表：**由 tools/gen_pinyin.py 生成，不要手改**。\n"
        "//!\n"
        "//! 每行「汉字 空格 读音1,读音2」；多音字全收。\n"
        "//! 数据来自 pypinyin 0.55（拼音数据本身为 MIT/CC 授权的公开数据），\n"
        "//! 只在开发期生成一次，运行时是纯静态字符串，不依赖 Python。\n"
        "//!\n"
        "//! 覆盖 U+4E00–U+9FFF，共 %d 字。\n\n" % len(lines)
    )
    source = header + "pub static TABLE: &str = concat!(\n" + "".join(chunks) + ");\n"
    io.open(OUT, "w", encoding="utf-8", newline="\n").write(source)
    print("wrote %s: %d chars, %d bytes" % (OUT, len(lines), len(source.encode("utf-8"))))


if __name__ == "__main__":
    main()
