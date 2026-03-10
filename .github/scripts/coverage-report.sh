#!/usr/bin/env bash
# Generate a unified Markdown coverage report (Rust + Frontend) and post as PR comment.
# Usage: coverage-report.sh <rust-base.lcov> <rust-pr.lcov> <frontend-base.lcov> <frontend-pr.lcov>
#
# Compatible with bash 3.2+ (macOS default) — no associative arrays.
set -euo pipefail
export LC_NUMERIC=C

RUST_BASE="${1:-}"
RUST_PR="${2:-}"
FE_BASE="${3:-}"
FE_PR="${4:-}"

# ── Parse lcov file → "lines_hit lines_total" ───────────────────────────
parse_lcov_totals() {
    local file="$1"
    if [[ ! -f "$file" || ! -s "$file" ]]; then echo "0 0"; return; fi
    awk -F: '
        /^LH:/ { hit  += $2 }
        /^LF:/ { total += $2 }
        END { print (hit+0), (total+0) }
    ' "$file"
}

# ── Parse lcov file → per-file TSV (file\thit\ttotal) ──────────────────
parse_lcov_files() {
    local file="$1"
    if [[ ! -f "$file" || ! -s "$file" ]]; then return; fi
    awk -F: '
        /^SF:/ { sf = $2 }
        /^LH:/ { lh = $2 }
        /^LF:/ { lf = $2 }
        /^end_of_record/ {
            if (lf > 0) printf "%s\t%s\t%s\n", sf, lh, lf
            sf = ""; lh = 0; lf = 0
        }
    ' "$file"
}

# ── Formatting helpers ──────────────────────────────────────────────────
calc_pct() {
    local hit=$1 total=$2
    if [[ "$total" -eq 0 ]]; then echo "0.0"; else echo "scale=1; $hit * 100 / $total" | bc -l; fi
}

pct_icon() {
    local pct="$1"
    if (( $(echo "$pct >= 90" | bc -l) )); then echo "🟢"
    elif (( $(echo "$pct >= 70" | bc -l) )); then echo "🟡"
    else echo "🔴"
    fi
}

diff_str() {
    local d="$1"
    if (( $(echo "$d > 0" | bc -l) )); then echo "+${d}%"
    elif (( $(echo "$d < 0" | bc -l) )); then echo "${d}%"
    else echo "±0%"
    fi
}

progress_bar() {
    local pct="$1"
    local filled=$(printf "%.0f" "$(echo "$pct / 5" | bc -l)")
    (( filled > 20 )) && filled=20
    local empty=$((20 - filled))
    local bar=""
    local i
    for (( i=0; i<filled; i++ )); do bar+="█"; done
    for (( i=0; i<empty; i++ )); do bar+="░"; done
    echo "$bar"
}

# ── Compute per-segment totals ──────────────────────────────────────────
read -r rust_pr_hit rust_pr_total <<< "$(parse_lcov_totals "$RUST_PR")"
read -r rust_base_hit rust_base_total <<< "$(parse_lcov_totals "$RUST_BASE")"
read -r fe_pr_hit fe_pr_total <<< "$(parse_lcov_totals "$FE_PR")"
read -r fe_base_hit fe_base_total <<< "$(parse_lcov_totals "$FE_BASE")"

all_pr_hit=$((rust_pr_hit + fe_pr_hit))
all_pr_total=$((rust_pr_total + fe_pr_total))
all_base_hit=$((rust_base_hit + fe_base_hit))
all_base_total=$((rust_base_total + fe_base_total))

all_pr_pct=$(calc_pct $all_pr_hit $all_pr_total)
all_base_pct=$(calc_pct $all_base_hit $all_base_total)
all_diff=$(echo "scale=1; $all_pr_pct - $all_base_pct" | bc -l)

rust_pr_pct=$(calc_pct $rust_pr_hit $rust_pr_total)
rust_base_pct=$(calc_pct $rust_base_hit $rust_base_total)
rust_diff=$(echo "scale=1; $rust_pr_pct - $rust_base_pct" | bc -l)

fe_pr_pct=$(calc_pct $fe_pr_hit $fe_pr_total)
fe_base_pct=$(calc_pct $fe_base_hit $fe_base_total)
fe_diff=$(echo "scale=1; $fe_pr_pct - $fe_base_pct" | bc -l)

# ── Save per-file data to temp files for lookup ─────────────────────────
PR_FILES_RUST=$(mktemp)
PR_FILES_FE=$(mktemp)
trap 'rm -f "$PR_FILES_RUST" "$PR_FILES_FE"' EXIT

parse_lcov_files "$RUST_PR" > "$PR_FILES_RUST"
parse_lcov_files "$FE_PR" > "$PR_FILES_FE"

# Lookup: file_coverage <prefix-to-strip> <search-key> <tmpfile>
# Returns "hit total" or empty
file_coverage() {
    local key="$1" tmpfile="$2"
    awk -F'\t' -v k="$key" '$1 ~ k { print $2, $3; exit }' "$tmpfile"
}

# ── Generate Markdown ───────────────────────────────────────────────────
REPORT="<!-- coverage-report -->
## $(pct_icon "$all_pr_pct") Code Coverage: ${all_pr_pct}% ($(diff_str "$all_diff"))

$(progress_bar "$all_pr_pct") **${all_pr_pct}%** — ${all_pr_hit}/${all_pr_total} lines covered

| Segment | Base | PR | Diff |
|---------|-----:|---:|-----:|
| 🦀 **Rust** (${rust_pr_hit}/${rust_pr_total}) | ${rust_base_pct}% | ${rust_pr_pct}% | $(diff_str "$rust_diff") |
| 🟩 **Frontend** (${fe_pr_hit}/${fe_pr_total}) | ${fe_base_pct}% | ${fe_pr_pct}% | $(diff_str "$fe_diff") |
| **Total** | ${all_base_pct}% | ${all_pr_pct}% | $(diff_str "$all_diff") |
"

# ── Changed files table ─────────────────────────────────────────────────
changed_files=""
if [[ -n "${GITHUB_EVENT_PATH:-}" ]]; then
    pr_number=$(jq -r '.pull_request.number // empty' "$GITHUB_EVENT_PATH" 2>/dev/null || true)
    if [[ -n "$pr_number" ]]; then
        changed_files=$(gh pr diff "$pr_number" --name-only 2>/dev/null || true)
    fi
fi

if [[ -n "$changed_files" ]]; then
    has_changed=false
    changed_table=""

    while IFS= read -r f; do
        [[ -z "$f" ]] && continue
        cov=""
        # Try Rust (strip src-tauri/ prefix from changed file, match in lcov paths)
        if [[ "$f" == src-tauri/* ]]; then
            short="${f#src-tauri/}"
            cov=$(file_coverage "$short" "$PR_FILES_RUST")
        elif [[ "$f" == src/* ]]; then
            short="${f#src/}"
            cov=$(file_coverage "$short" "$PR_FILES_FE")
        fi
        if [[ -n "$cov" ]]; then
            read -r fh ft <<< "$cov"
            fp=$(calc_pct "$fh" "$ft")
            changed_table+="
| $(pct_icon "$fp") \`${f}\` | ${fh}/${ft} | ${fp}% |"
            has_changed=true
        fi
    done <<< "$changed_files"

    if $has_changed; then
        REPORT+="
<details><summary>📂 Changed files coverage</summary>

| File | Lines | Coverage |
|------|------:|--------:|${changed_table}
</details>"
    fi
fi

# ── Low coverage files (< 50%, min 10 lines) ───────────────────────────
low_table=""
while IFS=$'\t' read -r sf lh lf; do
    [[ -z "$sf" ]] && continue
    [[ "$lf" -lt 10 ]] && continue
    pct=$(calc_pct "$lh" "$lf")
    if (( $(echo "$pct < 50" | bc -l) )); then
        display="${sf#*/src-tauri/}"
        low_table+="
| $(pct_icon "$pct") \`${display}\` | ${lh}/${lf} | ${pct}% |"
    fi
done < "$PR_FILES_RUST"

while IFS=$'\t' read -r sf lh lf; do
    [[ -z "$sf" ]] && continue
    [[ "$lf" -lt 10 ]] && continue
    pct=$(calc_pct "$lh" "$lf")
    if (( $(echo "$pct < 50" | bc -l) )); then
        display="${sf#*/src/}"
        low_table+="
| $(pct_icon "$pct") \`${display}\` | ${lh}/${lf} | ${pct}% |"
    fi
done < "$PR_FILES_FE"

if [[ -n "$low_table" ]]; then
    REPORT+="

<details><summary>⚠️ Files below 50% coverage</summary>

| File | Lines | Coverage |
|------|------:|--------:|${low_table}
</details>"
fi

REPORT+="

---
*Generated by [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) + [vitest](https://vitest.dev)*"

# ── Post or update PR comment ───────────────────────────────────────────
echo "$REPORT"

if [[ -n "${GITHUB_EVENT_PATH:-}" ]]; then
    pr_number=$(jq -r '.pull_request.number // empty' "$GITHUB_EVENT_PATH" 2>/dev/null || true)
    if [[ -n "$pr_number" ]]; then
        existing_id=$(gh api "repos/${GITHUB_REPOSITORY}/issues/${pr_number}/comments" \
            --jq '.[] | select(.body | contains("<!-- coverage-report -->")) | .id' \
            2>/dev/null | head -1 || true)

        if [[ -n "$existing_id" ]]; then
            gh api "repos/${GITHUB_REPOSITORY}/issues/comments/${existing_id}" \
                -X PATCH -f body="$REPORT" > /dev/null
            echo "Updated existing coverage comment #${existing_id}"
        else
            gh pr comment "$pr_number" --body "$REPORT"
            echo "Posted new coverage comment"
        fi
    fi
fi
