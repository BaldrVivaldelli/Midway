import sys


def test_lines(path):
    lines = open(path, encoding="utf-8").read().splitlines()
    total = len(lines)
    i = 0
    test = 0
    spans = []
    while i < len(lines):
        if lines[i].strip() == "#[cfg(test)]":
            start = i
            depth = 0
            opened = False
            j = i
            while j < len(lines):
                depth += lines[j].count("{") - lines[j].count("}")
                if "{" in lines[j]:
                    opened = True
                if opened and depth <= 0:
                    break
                j += 1
            spans.append((start + 1, j + 1))
            test += (j - start + 1)
            i = j + 1
        else:
            i += 1
    return total, test, spans


for path in sys.argv[1:]:
    total, test, spans = test_lines(path)
    print(f"{path}: total={total} test={test} prod={total - test} spans={spans}")
