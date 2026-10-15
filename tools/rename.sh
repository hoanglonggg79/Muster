#!/usr/bin/env bash

set -euo pipefail


die() {
  printf 'loi: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '\033[36m==>\033[0m %s\n' "$*"
}

warn() {
  printf '\033[33mcanh bao:\033[0m %s\n' "$*" >&2
}

ok() {
  printf '\033[32m  ok\033[0m %s\n' "$*"
}

lower() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]'
}


if [ $# -ne 1 ]; then
  printf 'dung: %s <TenMoi>\n\n' "$(basename "$0")" >&2
  printf 'vi du: %s PhantomX\n' "$(basename "$0")" >&2
  exit 2
fi

NEW_DISPLAY="$1"

case "$NEW_DISPLAY" in
  *[!A-Za-z0-9]*) die "ten moi chi duoc chua chu va so (khong dau cach, khong dau gach): '$NEW_DISPLAY'" ;;
esac
case "$NEW_DISPLAY" in
  [0-9]*) die "ten moi phai bat dau bang chu cai: '$NEW_DISPLAY'" ;;
esac
[ "${#NEW_DISPLAY}" -ge 2 ] || die "ten moi qua ngan: '$NEW_DISPLAY'"

NEW_SLUG="$(lower "$NEW_DISPLAY")"
NEW_UPPER="$(printf '%s' "$NEW_DISPLAY" | tr '[:lower:]' '[:upper:]')"


SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
MANIFEST="$ROOT/Cargo.toml"

[ -f "$MANIFEST" ] || die "khong tim thay $MANIFEST"

OLD_DISPLAY="$(sed -n 's/^[[:space:]]*display[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$MANIFEST" | head -1)"
OLD_SLUG="$(sed -n 's/^[[:space:]]*codename[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$MANIFEST" | head -1)"

[ -n "$OLD_DISPLAY" ] || die "khong doc duoc [workspace.metadata.project] display trong Cargo.toml"
[ -n "$OLD_SLUG" ] || die "khong doc duoc [workspace.metadata.project] codename trong Cargo.toml"

OLD_UPPER="$(printf '%s' "$OLD_DISPLAY" | tr '[:lower:]' '[:upper:]')"

info "ten hien tai : $OLD_DISPLAY  (codename: $OLD_SLUG)"
info "ten moi      : $NEW_DISPLAY  (codename: $NEW_SLUG)"

if [ "$OLD_DISPLAY" = "$NEW_DISPLAY" ]; then
  die "ten moi trung voi ten hien tai — khong co gi de doi"
fi

case "$NEW_SLUG" in
  *"$OLD_SLUG"*) die "codename moi '$NEW_SLUG' chua codename cu '$OLD_SLUG' — chon ten khac" ;;
esac


if [ -d "$ROOT/.git" ] && [ "${RENAME_YES:-0}" != "1" ]; then
  if [ -n "$(git -C "$ROOT" status --porcelain 2>/dev/null)" ]; then
    warn "cay lam viec dang co thay doi chua commit."
    warn "hay commit truoc khi doi ten de con doi chieu duoc bang 'git diff'."
    warn "hoac dat RENAME_YES=1 de bo qua canh bao nay."
    printf 'tiep tuc? [y/N] '
    read -r answer
    case "$answer" in
      [yY]*) ;;
      *) die "da huy" ;;
    esac
  fi
fi


info "buoc 1/5 — thay ten trong noi dung file"

mapfile -t TARGETS < <(
  grep -rIl \
    --exclude-dir=.git \
    --exclude-dir=target \
    --exclude-dir=node_modules \
    --exclude-dir=dist \
    --exclude-dir=RAM_Source \
    --exclude-dir=.workbuddy-ai \
    --exclude-dir=gen \
    --exclude='Cargo.lock' \
    --exclude='package-lock.json' \
    -e "$OLD_DISPLAY" -e "$OLD_UPPER" -e "$OLD_SLUG" \
    "$ROOT" 2>/dev/null || true
)

if [ "${#TARGETS[@]}" -eq 0 ]; then
  warn "khong tim thay file nao chua '$OLD_DISPLAY' / '$OLD_SLUG'"
else
  for f in "${TARGETS[@]}"; do
    sed -i \
      -e "s/${OLD_DISPLAY}/${NEW_DISPLAY}/g" \
      -e "s/${OLD_UPPER}/${NEW_UPPER}/g" \
      -e "s/${OLD_SLUG}/${NEW_SLUG}/g" \
      "$f"
  done
  ok "da sua ${#TARGETS[@]} file"
fi


info "buoc 2/5 — doi ten thu muc crate"

shopt -s nullglob
for dir in "$ROOT"/crates/"$OLD_SLUG"-*; do
  [ -d "$dir" ] || continue
  base="$(basename "$dir")"
  suffix="${base#"$OLD_SLUG"-}"
  target="$ROOT/crates/$NEW_SLUG-$suffix"
  [ -e "$target" ] && die "dich da ton tai: $target"
  mv "$dir" "$target"
  ok "crates/$base -> crates/$NEW_SLUG-$suffix"
done
shopt -u nullglob


info "buoc 3/5 — cap nhat [workspace.metadata.project]"

sed -i \
  -e "s/^\([[:space:]]*display[[:space:]]*=[[:space:]]*\)\"[^\"]*\"/\1\"${NEW_DISPLAY}\"/" \
  -e "s/^\([[:space:]]*codename[[:space:]]*=[[:space:]]*\)\"[^\"]*\"/\1\"${NEW_SLUG}\"/" \
  "$MANIFEST"
ok "display = \"$NEW_DISPLAY\", codename = \"$NEW_SLUG\""


info "buoc 4/5 — sinh lai Cargo.lock"

rm -f "$ROOT/Cargo.lock"
if (cd "$ROOT" && cargo generate-lockfile -q 2>/dev/null); then
  ok "Cargo.lock da sinh lai"
else
  warn "khong sinh duoc Cargo.lock — chay tay: cargo generate-lockfile"
fi

APP_DIR="$ROOT/crates/$NEW_SLUG-app"
if [ -f "$APP_DIR/package-lock.json" ]; then
  if [ -d "$APP_DIR/node_modules" ] &&
     (cd "$APP_DIR" && npm install --package-lock-only --silent >/dev/null 2>&1); then
    ok "package-lock.json da lam moi"
  else
    warn "package-lock.json con ten cu — chay 'npm install' trong crates/$NEW_SLUG-app"
  fi
fi

if (cd "$ROOT" && cargo clean \
      -p "$NEW_SLUG-core" -p "$NEW_SLUG-api" -p "$NEW_SLUG-cli" -p "$NEW_SLUG-app" \
      >/dev/null 2>&1); then
  ok "da don artifact cu cua crate (tranh ICE do cache incremental)"
fi


if [ "${RENAME_SKIP_VERIFY:-0}" = "1" ]; then
  info "buoc 5/5 — kiem chung: BO QUA (RENAME_SKIP_VERIFY=1)"
  printf '\n'
  printf '\033[33mdoi ten xong nhung CHUA duoc kiem chung.\033[0m\n'
  printf 'hay chay tay: cargo fmt --all -- --check && cargo test && cargo check --workspace\n'
  exit 0
fi

info "buoc 5/5 — kiem chung"

FAILED=0

if (cd "$ROOT" && cargo fmt --all -- --check >/dev/null 2>&1); then
  ok "cargo fmt --check"
else
  warn "cargo fmt --check khong sach"
  FAILED=1
fi

if (cd "$ROOT" && cargo test -q >/dev/null 2>&1); then
  ok "cargo test (3 crate nhe)"
else
  warn "cargo test that bai"
  FAILED=1
fi

if (cd "$ROOT" && cargo check --workspace -q >/dev/null 2>&1); then
  ok "cargo check --workspace"
else
  warn "cargo check --workspace that bai"
  FAILED=1
fi

if [ -d "$ROOT/crates/$NEW_SLUG-app" ]; then
  if (cd "$ROOT/crates/$NEW_SLUG-app" && npx tsc --noEmit >/dev/null 2>&1); then
    ok "tsc --noEmit"
  else
    warn "tsc --noEmit that bai (da chay npm install chua?)"
    FAILED=1
  fi
fi


printf '\n'
if [ "$FAILED" -eq 0 ]; then
  printf '\033[32mdoi ten xong:\033[0m %s  ->  %s\n' "$OLD_DISPLAY" "$NEW_DISPLAY"
else
  printf '\033[31mdoi ten xong nhung con loi kiem chung.\033[0m\n'
fi

cat <<EOF

Con phai lam tay:
  - ROADMAP.md muc 10.3 va README.md: cau "codename tam la <ten cu>" gio khong con dung nua.
  - Doi ten thu muc goc cua repo neu muon (script co tinh KHONG lam viec nay).
  - .workbuddy-ai/ khong bi sua — ghi chu cu van dung ten cu.
  - Kiem tra 'git diff' de chac chan RAM_Source/ khong bi cham vao.
EOF

exit "$FAILED"
