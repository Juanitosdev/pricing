# Price Comparison — local venue-snapshot dashboard

A single-folder, no-backend, no-network price comparison tool. Open it in a
browser and compare unit prices for the same product across venues, from one
sales snapshot per venue. Everything runs in the page — no server, no database,
no AI calls. The same input always produces the same output.

## Run it

**Easiest (double-click):**

1. Open `index.html` in any modern browser.

It auto-loads the bundled snapshot from `catalogue.js` (a copy of
`sample-reports/catalogue.json`), so it works straight from the filesystem with
no server.

**Served (optional):** if you prefer a local server so `fetch` of
`sample-reports/catalogue.json` works too:

```
cd path/to/pricing
python -m http.server 8000
# then open http://localhost:8000/
```

## Folder layout

```
index.html                 the whole app (HTML + CSS + JS, no dependencies)
catalogue.js               bundled snapshot for offline auto-load (window.__CATALOGUE__)
sample-reports/
  catalogue.json           the processed data contract (array of rows, one per venue+PLU)
data/                      the raw POS reports the catalogue was built from
  PLU_Sales_With_Discounts_By_Dept_{venue}_{site}_{yyyymmdd}_{hhmmss}.csv
README.md
```

## The three screens

**Upload.** Drop the per-venue `PLU_Sales_With_Discounts_By_Dept_*.csv` reports
(or a ready-made `catalogue.json`). Filenames are parsed for venue, site and
date; a dropdown lets you correct the venue before anything is ingested. Nothing
is processed until you press **Confirm** — then the sectioned "By Dept" reports
are parsed and enriched client-side into the same row schema.

**Comparison.** Pick a product on the left. You get one panel per venue,
side by side. Inside each panel is one horizontal bar per price variant of that
product at that venue (standard, happy hour, tower, mixer…). Bar length is
proportional to price, scaled to the dearest variant across all venues so panels
are directly comparable. **Every bar shows the quantity sold under it** — a price
from 1 unit never looks like a price from 142.

- Variants are ordered by quantity descending — the one people actually buy first.
- Venues with no data for the product still render a panel, marked *not on the list*.
- `price_ambiguous` variants get a greyed, hatched bar and a struck-through price,
  with the tooltip *"averaged across more than one price — not a single tariff"*.
- When a venue has no exact match, a similarly-named product is shown with an amber
  *"matched from X — confirm"* note rather than being silently dropped.
- Filter the product list to **Comparable** (in ≥2 venues) or **Everything**.

**Review queue.** Every row where `needs_review` is true, grouped by flag, each with
a plain-English note on what the flag means and why it matters. A discount alone
never sends a row here — the unit price shown is always the gross, pre-discount
tariff.

## Data contract

`catalogue.json` is an array of rows, one per `(venue, PLU)`:

| field | notes |
|---|---|
| `venue`, `site`, `report_date` | `report_date` (`yyyymmdd`) is kept on every row; only the latest snapshot is displayed. |
| `plu`, `dept_no`, `dept` | POS identifiers. |
| `name_raw`, `name_norm` | raw and normalised product names; grouping is by `name_norm`. |
| `qty`, `net_value`, `discount`, `gross_value`, `avg_cost` | `net_value = gross_value − |discount|`. |
| `unit_price` | `gross_value / qty` (price **before** discounts). `null` when `qty = 0`. |
| `flags` | `no_sales, fractional_qty, has_discount, zero_price, price_ambiguous, happy_hour_variant, modifier, package_item, size_in_name` |
| `needs_review` | true when any flag other than `has_discount` / `size_in_name` (both informational) is present. |
| `group_id`, `group_venues`, `comparable` | `group_venues` is a **count**; `comparable` ⇔ `group_venues ≥ 2`. |

### How the flags are computed (deterministic)

The upload pipeline reproduces the same rules used to build the sample catalogue:

- **no_sales** — `qty = 0` (`unit_price` is `null`, shown as *no price*).
- **zero_price** — `qty > 0` but `gross_value = 0` (`unit_price` is `0`, shown as *no price*).
- **fractional_qty** — `qty` is not a whole number (sold by measure).
- **has_discount** — `discount ≠ 0` (informational; never triggers review).
- **price_ambiguous** — the unit price doesn't land on a 5p menu step
  (`unit_price` is not a whole multiple of £0.05), i.e. the line blends more than
  one price and the figure is an average.
- **happy_hour_variant** — name contains `HH` / `Happy Hour`.
- **size_in_name** — name encodes a size (`…ml`, `Tower`, `Btl`).
- **package_item** — name starts with `Btm` / `Brz` (bottomless / bundle).
- **modifier** — name starts with `+` (an add-on, not a product).

## Design rules honoured

- A price is never shown without the quantity behind it.
- `price_ambiguous` is struck-through / greyed with the ambiguity tooltip.
- `no_sales` / `zero_price` read *no price*, never `0.00`.
- No line charts — one upload is one snapshot; comparison is across venues, not time.
- Deterministic: the same file always produces the same output.
