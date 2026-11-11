#!/usr/bin/env python3
"""T1018 : génère docs/CATALOGUE.md depuis target/catalogue-report.md (banc T1004) et defs.rs.

Usage : python tools/gen_catalogue_md.py [--report target/catalogue-report.md] [--out docs/CATALOGUE.md]
        python tools/gen_catalogue_md.py --selftest
Les remarques ci-dessous sont les seuls commentaires saisis (écarts connus, voir docs/STATE.md).
"""
import os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

NOTES = {
    "kof2000": "déroulement en retard sur MAME après ~580 trames (voir QUESTIONS.md)",
    "kof2001": "déroulement en retard sur MAME après ~580 trames (voir QUESTIONS.md)",
    "kof99": "niveau de fondu différent de MAME (voir QUESTIONS.md)",
    "fatfury2": "même écran que MAME, niveau de fondu différent",
    "ssideki": "même écran que MAME, niveau de fondu différent",
    "jockeygp": "écran de mise au lieu du titre (voir QUESTIONS.md, T1014)",
    "irrmaze": "BIOS spécial du zip ; pas de golden comparable (autre résolution) ; jeu actif à 1500 trames",
    "mslug3": "écart connu à 1500 trames (diff 82,79 %)",
    "garou": "à 1500 trames : diff 31,97 % (déroulement différent de MAME)",
    "fatfury3": "à 1500 trames : diff 88,60 % (déroulement différent de MAME)",
    "kof94": "à 1500 trames : écran sombre (8 couleurs), pas de golden à 1500 (MAME bloque)",
    "rbff2": "à 1500 trames : diff 2,23 % (juste au-dessus du seuil de 2 %)",
    "lastblad": "déroulement en retard sur MAME (voir QUESTIONS.md)",
    "samsho4": "déroulement en retard sur MAME (voir QUESTIONS.md)",
    "kof96": "déroulement en retard sur MAME (voir QUESTIONS.md)",
    "cnbe": "homebrew (démo) : peu de couleurs, tourne",
    "ltorb": "homebrew : peu de couleurs, tourne",
    "neothund": "homebrew : peu de couleurs, tourne",
    "ngem2k": "homebrew : peu de couleurs, tourne",
}


def parse_defs(text):
    """[(nom, titre, protection, contrôleur)] dans l'ordre de ALL_CARTS."""
    out = []
    for m in re.finditer(r'name: "([^"]+)", title: "([^"]*)", parts: \w+,\s*\n.*?\n.*?\n\s*prot: PROT_(\w+),\s*\n\s*controller: Controller::(\w+),', text):
        out.append((m.group(1), m.group(2), m.group(3), m.group(4)))
    return out


def parse_report(text):
    rows = {}
    for line in text.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) >= 5 and cells[0] not in ("jeu", "---") and not cells[0].startswith("-"):
            rows[cells[0]] = cells
    return rows


def render(defs, report, report1500=None):
    report1500 = report1500 or {}
    out = ["# Catalogue des jeux", "",
           "Généré par `tools/gen_catalogue_md.py` (banc `cargo test -p ng-core --test catalogue -- --ignored`).", "",
           "| jeu | titre | protection | contrôleur | état | diff120 | diff600 | à 1500 trames | remarques |",
           "|---|---|---|---|---|---|---|---|---|"]
    for name, title, prot, ctl in sorted(defs):
        r = report.get(name)
        etat, d120, d600 = (r[1], r[2], r[3]) if r else ("non mesuré", "-", "-")
        r15 = report1500.get(name)
        if etat.startswith("noir") and r15 and r15[1].startswith("ok"):
            etat = "ok (à 1500 trames)"  # le BIOS est encore à l'écran à 600 trames
        # 1500 trames : la colonne « état » du banc contient déjà « diff X % » si une golden existe
        t15 = r15[1] if r15 else "-"
        out.append("| %s | %s | %s | %s | %s | %s | %s | %s | %s |" % (name, title, prot.lower(), ctl.lower(), etat, d120, d600, t15, NOTES.get(name, "")))
    out += ["", "## Hors périmètre", "",
            "- `powerins` : pas un jeu Neo Geo (carte arcade Jaleco).",
            "- `aes`, `neogeo`, `neocdz` : BIOS, pas des jeux.", ""]
    return "\n".join(out)


def selftest():
    defs_txt = ''
    for n in ("a", "b", "c"):
        defs_txt += 'pub static %s: CartDef = CartDef {\n    name: "%s", title: "Jeu %s", parts: P_%s,\n    sizes: X,\n    m1_reload: None,\n    prot: PROT_NONE,\n    controller: Controller::Joystick,\n};\n' % (n.upper(), n, n, n)
    defs = parse_defs(defs_txt)
    assert len(defs) == 3, defs
    rep = parse_report("| jeu | état | diff120 | diff finale | couleurs | secondes |\n|---|---|---|---|---|---|\n| a | ok | 0.00% | 0.00% | 30 | 1.0 |\n| b | noir | - | - | 5 | 1.0 |\n")
    rep15 = parse_report("| b | ok | - | - | 40 | 1.0 |\n| a | diff 5.00% | - | 5.00% | 9 | 1.0 |\n")
    md = render(defs, rep, rep15)
    lines = [l for l in md.splitlines() if l.startswith("|")]
    assert len(lines) == 3 + 2, lines  # 3 jeux + en-tête + séparateur
    assert "non mesuré" in lines[-1]
    assert "ok" in lines[3] and "diff 5.00%" in lines[2], lines
    print("selftest OK")


def main():
    if "--selftest" in sys.argv:
        return selftest()
    arg = lambda k, d: sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d
    report = arg("--report", os.path.join(ROOT, "target", "catalogue-report.md"))
    out = arg("--out", os.path.join(ROOT, "docs", "CATALOGUE.md"))
    defs = parse_defs(open(os.path.join(ROOT, "crates", "ng-cart", "src", "defs.rs"), encoding="utf-8").read())
    rep = parse_report(open(report, encoding="utf-8").read())
    r15 = arg("--report1500", None)
    rep15 = parse_report(open(r15, encoding="utf-8").read()) if r15 else None
    open(out, "w", encoding="utf-8").write(render(defs, rep, rep15))
    print("écrit", out, len(defs), "jeux")


if __name__ == "__main__":
    main()
