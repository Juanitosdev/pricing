#!/usr/bin/env python3
"""
Deterministic builder: Till Pricing CSV exports -> catalogue of products with measures.

CSV has real, header-labelled columns, so every field is mapped by name — no
position guessing, no truncated names, no merged numbers. A measure is derived
per L1 price level using the reliable SEL Unit label; only genuinely unlabelled
multi-price rows are sent to the review queue.

Departments / cross-venue pairing are the optional AI `enrich` step, not here.
Run: python scripts/build_catalogue.py   (reads data/*Pricing.csv)
"""
import csv, glob, os, re, json

DATA_DIR = "data"
OUT_JSON = "catalogue.json"
OUT_JS = "catalogue.js"

VENUE_LABEL = {"68": "68 & Shanghai", "Fam": "FAM", "Lucy": "LUCY", "Burlock": "Burlock", "TLT": "TLT"}
PRICE_COLS = ["Price1L1", "Price2L1", "Price3L1", "Price4L1"]

# optional AI-produced mapping: venue -> dept_code -> department name
DEPT_MAP = {}
if os.path.exists("dept_map.json"):
    try:
        DEPT_MAP = json.load(open("dept_map.json", encoding="utf-8"))
    except Exception:
        DEPT_MAP = {}

# optional AI-produced pairing: base_key -> canonical group id (merges same
# product across venues recorded under different names). Absent => group == base_key.
PAIR_MAP = {}
if os.path.exists("pairing_map.json"):
    try:
        PAIR_MAP = json.load(open("pairing_map.json", encoding="utf-8"))
    except Exception:
        PAIR_MAP = {}

SIZE_TOKEN = re.compile(r"(\d+\s?ml|\d+\s?cl|\bbtl\b|\bbottle\b|\btower\b|\bpint\b|\bpt\b|\bcan\b|\bjug\b|\bcarafe\b|\bmagnum\b|\bhalf\b|\bschooner\b)", re.I)
NOISE = re.compile(r"^(SPARE|PLU|ALLERGEN|IN ORDER|ALL TOGETHER|NEAT|TV Hire|Tarot|Crockery|Main & side|Small plates|Away|Supplement|Set Menu|Course|Random|COMP\b)", re.I)

def venue_from_filename(path):
    stem = re.sub(r"\s*Pricing\.csv$", "", os.path.basename(path), flags=re.I).strip()
    return VENUE_LABEL.get(stem, stem)

def num(t):
    try:
        return float(str(t).replace(",", "").replace("£", "").strip())
    except Exception:
        return None

def round2(x):
    return round(x + 1e-9, 2)

def size_token(name):
    m = SIZE_TOKEN.search(name)
    return m.group(1).strip().lower() if m else ""

def norm_serve(alt):
    """Map the messy Alt-Mod serving text to a canonical serving kind."""
    a = (alt or "").lower().strip()
    if not a: return ""
    if re.search(r"½\s*pt|half|1/2\s*pt", a): return "half"
    if re.search(r"pint|\bpt\b", a): return "pint"
    if re.search(r"double|\bdbl\b", a): return "double"
    if re.search(r"single|\bsgl\b|shot", a): return "single"
    if re.search(r"mixer", a): return "mixer"
    if re.search(r"bottle|\bbtl\b|^b$", a): return "bottle"
    return ""

BUCKET = re.compile(r"bucket\s*of\s*(\d+)|(\d+)\s*for\b|\bx\s*(\d+)\b", re.I)
def bucket_qty(name):
    m = BUCKET.search(name)
    if not m: return None
    for g in m.groups():
        if g: return int(g)
    return None

def derive_measures(prices, sel, name):
    """prices: the 4 Price*L1 values. sel: canonical serving from norm_serve. -> (measures, flags)."""
    flags = []
    P = [round2(p) for p in prices if p and p > 0]
    n = len(P)
    serve = norm_serve(sel)
    st = size_token(name)
    if n == 0:
        return [], flags
    bq = bucket_qty(name)
    if bq and n == 1:
        flags.append("package")
        return [{"label": f"bucket ×{bq}", "price": P[0]}], flags
    if n == 1:
        lbl = st or ("bottle" if serve == "bottle" else "each")
        ms = [{"label": lbl, "price": P[0]}]
    elif n == 2:
        if serve == "half":
            ms = [{"label": "pint", "price": max(P)}, {"label": "half", "price": min(P)}]
        elif serve == "pint":
            ms = [{"label": "pint", "price": max(P)}, {"label": "half", "price": min(P)}]
        elif serve in ("single", "double"):
            ms = [{"label": "single", "price": min(P)}, {"label": "double", "price": max(P)}]
        elif serve == "mixer":
            ms = [{"label": "unit", "price": max(P)}, {"label": "mixer", "price": min(P)}]
        else:
            ms = [{"label": "large", "price": max(P)}, {"label": "small", "price": min(P)}]
            flags.append("measure_ambiguous")
    elif n == 3:
        s = sorted(P)  # ascending
        if serve in ("single", "double") or (s[2] >= 60 and s[2] >= 4 * s[1]):
            ms = [{"label": "single", "price": s[0]},
                  {"label": "double", "price": s[1]},
                  {"label": "bottle", "price": s[2]}]
            if serve not in ("single", "double"):
                flags.append("measure_ambiguous")
        elif serve in ("half", "pint"):
            ms = [{"label": "pint", "price": s[2]}, {"label": "half", "price": s[1]},
                  {"label": "other", "price": s[0]}]
            flags.append("measure_ambiguous")
        else:                                               # wine bottle/250/175
            ms = [{"label": "bottle", "price": s[2]},
                  {"label": "250ml", "price": s[1]},
                  {"label": "175ml", "price": s[0]}]
    else:                                                   # n == 4 -> wine bottle/250/175/125
        s = sorted(P, reverse=True)
        ms = [{"label": "bottle", "price": s[0]}, {"label": "250ml", "price": s[1]},
              {"label": "175ml", "price": s[2]}, {"label": "125ml", "price": s[3]}]
    # sanity: implausible magnitudes, or a 2-serving item whose two prices are
    # wildly apart (a single/double or pint/half can't differ ~8x) => source typo
    for m in ms:
        if m["price"] > 2000 or m["price"] < 0.05:
            flags.append("price_suspect"); break
    if n == 2 and min(P) > 0 and max(P) / min(P) > 8:
        flags.append("price_suspect")
    return ms, sorted(set(flags))

def product_flags(name):
    f = []
    if re.match(r"^\s*\+", name): f.append("modifier")
    if re.match(r"^(brz|btm)\b", name, re.I): f.append("package")
    if re.search(r"\b(hh|happy\s*hour)\b", name, re.I): f.append("happy_hour")
    if SIZE_TOKEN.search(name): f.append("size_in_name")
    return f

def name_norm(s):
    return re.sub(r"\s+", " ", s.lower()).strip()

def base_key(nn):
    s = " " + nn + " "
    s = re.sub(r"\s\+\s*", " ", s)
    s = re.sub(r"\b(hh|happy\s*hour)\b", " ", s)
    s = re.sub(r"\b\d+\s?(ml|cl|l)\b", " ", s)
    s = re.sub(r"\b(tower|btl|bottle|pint|pt|can|half|double|single|sgl|magnum|glass|carafe|litre|schooner)\b", " ", s)
    s = re.sub(r"\s+", " ", s).strip()
    return s or nn.strip()

def build_products(path):
    venue = venue_from_filename(path)
    out = []
    with open(path, encoding="utf-8-sig", newline="") as fh:
        for row in csv.DictReader(fh):
            name = (row.get("Name") or "").strip()
            if not name or len(name) < 2 or NOISE.match(name):
                continue
            if "xxx" in name.lower():   # hidden / disabled till buttons
                continue
            prices = [num(row.get(c)) or 0.0 for c in PRICE_COLS]
            if not any(p and p > 0 for p in prices):
                continue
            # serving label ("Half" / "Double" / "Single") lives in Alt Mod Text,
            # NOT "SEL Unit" (which is a numeric code)
            sel = (row.get("Alt Mod Text 1") or row.get("Alt Mod Text 2") or "").strip()
            measures, mflags = derive_measures(prices, sel, name)
            if not measures:
                continue
            flags = sorted(set(product_flags(name) + mflags))
            nn = name_norm(name)
            bk = base_key(nn)
            dept_code = (row.get("Department Link") or "").strip()
            out.append({
                "venue": venue, "name_raw": name, "name_norm": nn, "base_key": bk,
                "group": PAIR_MAP.get(bk, bk),
                "dept_code": dept_code,
                "department": DEPT_MAP.get(venue, {}).get(dept_code, "Other"),
                "measures": measures, "flags": flags,
                "needs_review": any(x in flags for x in ("measure_ambiguous", "price_suspect")),
            })
    return venue, out

def main():
    catalogue, counts = [], []
    for path in sorted(glob.glob(os.path.join(DATA_DIR, "*Pricing.csv"))):
        venue, prods = build_products(path)
        counts.append((venue, len(prods)))
        catalogue.extend(prods)
    with open(OUT_JSON, "w", encoding="utf-8") as f:
        json.dump(catalogue, f, ensure_ascii=False, indent=1)
    with open(OUT_JS, "w", encoding="utf-8") as f:
        f.write("window.__CATALOGUE__ = " + json.dumps(catalogue, ensure_ascii=False) + ";\n")

    print("venue                products")
    for v, n in counts: print(f"  {v:<18}{n:>8}")
    rev = sum(1 for p in catalogue if p["needs_review"])
    print(f"TOTAL {len(catalogue)} products, {rev} need review ({100*rev//max(1,len(catalogue))}%)")

    def show(pred, title, limit=7):
        print(f"\n== {title} ==")
        seen = 0
        for p in catalogue:
            if pred(p):
                ms = "  ".join(f"{m['label']} GBP{m['price']}" for m in p["measures"])
                fl = (" [" + ",".join(p["flags"]) + "]") if p["flags"] else ""
                print(f"  {p['venue']:<14}{p['name_raw']:<30}{ms}{fl}")
                seen += 1
                if seen >= limit: break
    show(lambda p: re.search(r"^kingfisher$", p["name_raw"], re.I), "Kingfisher (expect pint/half)")
    show(lambda p: re.search(r"grey goose", p["name_raw"], re.I), "Grey Goose across venues (was £670 in LUCY)")
    show(lambda p: re.search(r"nardini", p["name_raw"], re.I), "Nardini (was £1310)")
    show(lambda p: len(p["measures"]) == 4, "wines (bottle/250/175/125)")
    show(lambda p: re.search(r"macallan 18", p["name_raw"], re.I), "Macallan 18 (premium — should be high but sane)")

if __name__ == "__main__":
    main()
