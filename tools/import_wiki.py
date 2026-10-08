#!/usr/bin/env python3
"""One-off import of the GitHub wiki into content/.

Usage: tools/import_wiki.py <wiki-dir> <content-dir>

Kept for provenance: it records how the wiki pages became content/ pages. Pages are imported
valid for every version; version-specific parts are annotated by hand afterwards.
"""
import re
import sys
from pathlib import Path

# slug: (wiki page, section, order, nav label, title override)
PAGES = [
    ("home", "Home", "Start", 0, "Overview", "Minigraf documentation"),
    ("tutorial-setup", "Tutorial-Setup", "Tutorial", 0, "Setup", None),
    ("tutorial-01-basic-transact-query", "Tutorial-01-Basic-Transact-Query", "Tutorial", 1, "1. Basic transact + query", None),
    ("tutorial-02-as-of", "Tutorial-02-As-Of", "Tutorial", 2, "2. `:as-of`", None),
    ("tutorial-03-valid-at", "Tutorial-03-Valid-At", "Tutorial", 3, "3. `:valid-at` / `:any-valid-time`", None),
    ("tutorial-04-recursive-rules", "Tutorial-04-Recursive-Rules", "Tutorial", 4, "4. Recursive rules", None),
    ("tutorial-05-negation", "Tutorial-05-Negation", "Tutorial", 5, "5. Negation", None),
    ("tutorial-06-aggregates", "Tutorial-06-Aggregates", "Tutorial", 6, "6. Aggregates & windows", None),
    ("tutorial-07-expressions", "Tutorial-07-Expressions", "Tutorial", 7, "7. Expressions", None),
    ("tutorial-08-prepared-queries", "Tutorial-08-Prepared-Queries", "Tutorial", 8, "8. Prepared queries", None),
    ("tutorial-09-disjunction", "Tutorial-09-Disjunction", "Tutorial", 9, "9. Disjunction", None),
    ("tutorial-10-udfs", "Tutorial-10-UDFs", "Tutorial", 10, "10. User-defined functions", None),
    ("tutorial-11-marketplace", "Tutorial-11-Marketplace", "Tutorial", 11, "11. Marketplace", None),
    ("cookbook-graph-traversal", "Cookbook-Graph-Traversal", "Cookbook", 1, "Graph traversal", None),
    ("cookbook-time-travel", "Cookbook-Time-Travel", "Cookbook", 2, "Time travel", None),
    ("cookbook-bitemporal-modeling", "Cookbook-Bitemporal-Modeling", "Cookbook", 3, "Bitemporal modeling", None),
    ("cookbook-application-workflows", "Cookbook-Application-Workflows", "Cookbook", 4, "App workflows", None),
    ("datalog-reference", "Datalog-Reference", "Reference", 10, "Datalog reference", None),
    ("architecture", "Architecture", "Reference", 20, "Architecture", None),
    ("performance-tuning", "Performance-Tuning", "Reference", 30, "Performance tuning", None),
    ("use-cases", "Use-Cases", "Reference", 40, "Use cases", None),
    ("comparison", "Comparison", "Reference", 50, "Comparison", None),
    ("learning-resources", "Learning-Resources", "Reference", 70, "Learning resources", None),
]

WIKI_TO_SLUG = {wiki: slug for slug, wiki, *_ in PAGES}
REPO = "https://github.com/project-minigraf/minigraf"


def rewrite_links(text: str) -> str:
    def repl(m: re.Match) -> str:
        url = m.group(1)
        path, _, anchor = url.partition("#")
        if path.startswith(f"{REPO}/wiki/"):
            path = path[len(f"{REPO}/wiki/"):]
        if path in WIKI_TO_SLUG:
            url = WIKI_TO_SLUG[path] + (f"#{anchor}" if anchor else "")
        elif path.startswith("../blob/"):
            url = f"{REPO}/{path[3:]}" + (f"#{anchor}" if anchor else "")
        return f"]({url})"

    return re.sub(r"\]\(([^)\s]+)\)", repl, text)


def main() -> None:
    wiki, content = Path(sys.argv[1]), Path(sys.argv[2])
    content.mkdir(parents=True, exist_ok=True)
    for slug, page, section, order, nav, title in PAGES:
        text = (wiki / f"{page}.md").read_text()
        first, _, body = text.partition("\n")
        assert first.startswith("# "), page
        title = title or first[2:].strip()
        title = re.sub(r"^Section (\d+):", r"Tutorial \1:", title)
        front = f"---\ntitle: {title}\nnav: {nav}\nsection: {section}\norder: {order}\n---\n"
        (content / f"{slug}.md").write_text(front + rewrite_links(body.lstrip("\n")))


if __name__ == "__main__":
    main()
