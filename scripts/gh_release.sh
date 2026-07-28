#!/usr/bin/env bash
# scripts/gh_release.sh — ensure every crate's currently-published version
# has a matching git tag + GitHub release. Idempotent and self-healing: it
# doesn't parse release.sh's commit messages, it cross-checks crates.io
# directly, so it backfills correctly regardless of how (or whether) a past
# release was committed.
#
# For each crate: if its current Cargo.toml version is published on
# crates.io and has no `<crate>-vX.Y.Z` tag yet, find the commit that
# introduced that version line, tag it, push the tag, and create a GitHub
# release with a changelog of commits touching that crate since its
# previous tag.
#
# Scope: only the *current* version per crate. It does not backfill tags
# for older, superseded-but-still-published versions.
#
# Usage: ./scripts/gh_release.sh [--dry-run]
#
# release.sh calls this automatically after a successful (non-dry-run)
# publish run — see the --no-gh-release flag there to opt out.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATES_IO_USER_AGENT="gh_release.sh (net-lattice; https://github.com/F000NKKK/net-lattice)"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
info()   { echo -e "${CYAN}[gh-release]${RESET} $*"; }
ok()     { echo -e "${GREEN}[ok]${RESET} $*"; }
warn()   { echo -e "${YELLOW}[warn]${RESET} $*"; }
die()    { echo -e "${RED}[error]${RESET} $*" >&2; exit 1; }
dryrun() { echo -e "${YELLOW}[dry-run]${RESET} $*"; }

DRY_RUN=false
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        *) die "Неизвестный флаг: $arg" ;;
    esac
done

command -v gh >/dev/null 2>&1 || die "gh (GitHub CLI) не найден в PATH"
command -v git >/dev/null 2>&1 || die "git не найден"
cd "$WS"
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "не git-репозиторий: $WS"

crate_is_published() {
    local crate="$1" ver="$2" code
    code="$(curl -s -o /dev/null -w '%{http_code}' \
        -A "$CRATES_IO_USER_AGENT" \
        "https://crates.io/api/v1/crates/${crate}/${ver}" 2>/dev/null)" || code="000"
    [[ "$code" == "200" ]]
}

# ── Changelog: коммиты в диапазоне/пути, сгруппированные по Conventional
# Commits типу (feat/fix/refactor/perf/docs/test/chore/прочее).
categorize_notes() {
    local range="$1" path="$2"
    local feat="" fix="" refactor="" perf="" docs="" tests="" chore="" other=""

    while IFS=$'\t' read -r hash subject; do
        [[ -z "$hash" ]] && continue
        local line="- ${subject} (${hash})"$'\n'
        case "$subject" in
            feat*)      feat+="$line" ;;
            fix*)       fix+="$line" ;;
            refactor*)  refactor+="$line" ;;
            perf*)      perf+="$line" ;;
            docs*)      docs+="$line" ;;
            test*)      tests+="$line" ;;
            chore*)     chore+="$line" ;;
            *)          other+="$line" ;;
        esac
    done < <(git log --format='%h%x09%s' "$range" -- "$path" 2>/dev/null || true)

    local out=""
    [[ -n "$feat" ]]     && out+="### Features"$'\n\n'"$feat"$'\n'
    [[ -n "$fix" ]]      && out+="### Fixes"$'\n\n'"$fix"$'\n'
    [[ -n "$refactor" ]] && out+="### Refactor"$'\n\n'"$refactor"$'\n'
    [[ -n "$perf" ]]     && out+="### Performance"$'\n\n'"$perf"$'\n'
    [[ -n "$docs" ]]     && out+="### Docs"$'\n\n'"$docs"$'\n'
    [[ -n "$tests" ]]    && out+="### Tests"$'\n\n'"$tests"$'\n'
    [[ -n "$chore" ]]    && out+="### Chores"$'\n\n'"$chore"$'\n'
    [[ -n "$other" ]]    && out+="### Other"$'\n\n'"$other"$'\n'

    if [[ -z "$out" ]]; then
        echo "_Нет коммитов в \`${path}\` с предыдущего релиза этого крейта._"
    else
        printf '%s' "$out"
    fi
}

CREATED=0
SKIPPED_TAGGED=0
SKIPPED_UNPUBLISHED=0
NOT_FOUND=0

for toml in "$WS"/crates/*/Cargo.toml; do
    crate="$(grep -m1 '^name *= *"' "$toml" | sed 's/.*"\([^"]*\)".*/\1/')"
    version="$(grep -m1 '^version *= *"' "$toml" | sed 's/.*"\([^"]*\)".*/\1/')"
    [[ -n "$crate" && -n "$version" ]] || { warn "Не смог прочитать name/version из $toml"; continue; }

    tag="${crate}-v${version}"
    rel_path="crates/${crate}/Cargo.toml"

    if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
        SKIPPED_TAGGED=$((SKIPPED_TAGGED + 1))
        continue
    fi

    if ! crate_is_published "$crate" "$version"; then
        SKIPPED_UNPUBLISHED=$((SKIPPED_UNPUBLISHED + 1))
        continue
    fi

    sha="$(git log -1 --format='%H' -S"version = \"${version}\"" -- "$rel_path" 2>/dev/null || true)"
    if [[ -z "$sha" ]]; then
        warn "$tag: не нашёл коммит, где появилась version = \"${version}\" в $rel_path — пропускаю, тегни вручную"
        NOT_FOUND=$((NOT_FOUND + 1))
        continue
    fi

    info "Тега $tag нет, версия опубликована на crates.io — коммит ${sha:0:8}"

    prev_tag="$(git tag -l "${crate}-v*" --sort=-version:refname | grep -vF "$tag" | head -1 || true)"
    range="$sha"
    [[ -n "$prev_tag" ]] && range="${prev_tag}..${sha}"

    notes="$(categorize_notes "$range" "crates/${crate}/")"

    full_notes="## ${crate} v${version}

${notes}

crates.io: https://crates.io/crates/${crate}/${version}"

    if $DRY_RUN; then
        dryrun "git tag $tag ${sha:0:8} && git push origin $tag"
        dryrun "gh release create $tag --title \"${crate} v${version}\" --notes '<changelog: $(echo "$notes" | wc -l) commit(s)>'"
        if [[ -n "$prev_tag" ]]; then
            dryrun "  changelog range: ${prev_tag}..${sha:0:8}"
        else
            dryrun "  changelog range: (нет предыдущего тега) от начала истории до ${sha:0:8}"
        fi
    else
        git tag "$tag" "$sha"
        git push origin "$tag"
        gh release create "$tag" \
            --title "${crate} v${version}" \
            --notes "$full_notes"
        ok "Релиз создан: $tag"
    fi
    CREATED=$((CREATED + 1))
done

echo ""
if $DRY_RUN; then
    echo -e "${YELLOW}[dry-run]${RESET} Создал бы: $CREATED, уже с тегом: $SKIPPED_TAGGED, не опубликовано: $SKIPPED_UNPUBLISHED, не нашёл коммит: $NOT_FOUND"
else
    echo -e "${GREEN}${BOLD}Готово.${RESET} Создано релизов: $CREATED, уже было: $SKIPPED_TAGGED, не опубликовано: $SKIPPED_UNPUBLISHED, не нашёл коммит: $NOT_FOUND"
fi

[[ $NOT_FOUND -gt 0 ]] && exit 2
exit 0
