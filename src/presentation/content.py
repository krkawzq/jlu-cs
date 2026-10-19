"""Read the slide outline: chapters, layout hints, stepped screen copy."""
from dataclasses import dataclass, field
from pathlib import Path
import re

import yaml

FIELDS = ('版式', '屏幕文案', '设计说明', '依据')
STEP = '+++'


def plain(value: str) -> str:
    value = re.sub(r'\[([^\]]+)\]\((?:https?://|[\w./-]+\.(?:md|rs|py|pr|pri|json))[^)]*\)', r'\1', value)
    return value.replace('**', '').replace('`', '').strip()


@dataclass
class Block:
    kind: str
    text: str = ''
    rows: list[list[str]] | None = None
    language: str = ''
    label: str = ''
    step: int = 0


@dataclass
class Slide:
    id: str
    title: str
    chapter: str
    layout: str
    options: dict[str, str]
    fields: dict[str, str]
    blocks: list[Block]
    steps: int = field(default=0)


def blocks(screen: str) -> list[Block]:
    result = []
    lines = screen.splitlines()
    step = 0
    i = 0
    while i < len(lines):
        line = lines[i].strip()
        if not line:
            i += 1
            continue
        if line == STEP:
            step += 1
            i += 1
            continue
        if line.startswith('```'):
            descriptor = line[3:].split(' ', 1)
            language = descriptor[0]
            label = descriptor[1] if len(descriptor) > 1 else language
            code = []
            i += 1
            while i < len(lines) and not lines[i].strip().startswith('```'):
                code.append(lines[i])
                i += 1
            if i == len(lines):
                raise ValueError('代码块未闭合')
            result.append(Block('code', '\n'.join(code), language=language, label=label, step=step))
            i += 1
            continue
        if line.startswith('|'):
            rows = []
            while i < len(lines) and lines[i].strip().startswith('|'):
                cells = [plain(v) for v in lines[i].strip().strip('|').split('|')]
                if not all(re.fullmatch(r':?-+:?', v.replace(' ', '')) for v in cells):
                    rows.append(cells)
                i += 1
            if not rows or any(len(row) != len(rows[0]) for row in rows):
                raise ValueError('表格列数不一致')
            result.append(Block('table', rows=rows, step=step))
            continue
        if line.startswith('~ '):
            result.append(Block('focus', line[2:].strip(), step=step))
            i += 1
            continue
        bullet = re.match(r'^(?:- |\d+\. )(.*)', line)
        if bullet:
            result.append(Block('bullet', plain(bullet.group(1)), step=step))
        elif line.startswith('输出：'):
            result.append(Block('output', plain(line.removeprefix('输出：')), step=step))
        else:
            result.append(Block('emphasis' if line.startswith('**') and line.endswith('**') else 'text', plain(line), step=step))
        i += 1
    return result


def layout(value: str) -> tuple[str, dict[str, str]]:
    words = value.split()
    if not words:
        raise ValueError('版式不能为空')
    options = {}
    for word in words[1:]:
        key, sep, item = word.partition('=')
        if not sep:
            raise ValueError(f'版式选项应为 key=value：{word}')
        options[key] = item
    return words[0], options


def read_outline(path: Path) -> tuple[dict, list[Slide]]:
    source = path.read_text(encoding='utf-8')
    header = re.match(r'\A---\n(.*?)\n---\n', source, re.S)
    if not header:
        raise ValueError('文案缺少 YAML 元信息')
    metadata = yaml.safe_load(header.group(1))
    marks = list(re.finditer(r'^(#{1,2}) (\d{2}|A\d+|[^｜\n]+)｜(.+)$', source, re.M))
    slides = []
    chapter = ''
    for index, mark in enumerate(marks):
        if mark.group(1) == '#':
            chapter = mark.group(3).strip()
            continue
        if not re.fullmatch(r'\d{2}|A\d+', mark.group(2)):
            raise ValueError(f'页码格式错误：{mark.group(2)}')
        body = source[mark.end(): marks[index + 1].start() if index + 1 < len(marks) else len(source)]
        parts = re.split(r'^### (.+)\n', body, flags=re.M)
        fields = {parts[j].strip(): parts[j + 1].strip().removesuffix('---').strip() for j in range(1, len(parts), 2)}
        if tuple(fields) != FIELDS:
            raise ValueError(f'{mark.group(2)} 栏目应为 {FIELDS}')
        kind, options = layout(fields['版式'])
        items = blocks(fields['屏幕文案'])
        slides.append(Slide(mark.group(2), mark.group(3).strip(), chapter, kind, options, fields, items,
                            max((b.step for b in items), default=0)))
    ids = [slide.id for slide in slides]
    expected = [f'{n:02d}' for n in range(1, metadata['main_slides'] + 1)] + [f'A{n}' for n in range(1, metadata['appendix_slides'] + 1)]
    if ids != expected:
        raise ValueError(f'页码与元信息不一致：{ids}')
    return metadata, slides
