#!/usr/bin/env bash
# task:agent-eval-readme — M1 … M7.
#
# M7 is the point of the task — "a README that promises a check nobody wrote is a worse artefact
# than no README" — and it is the only row here that can be wrong in a way a reader cannot see.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

README="$EVAL_DIR/README.md"

declare_row M1 "the section follows the heading depth, ordering and layout of its two siblings"
declare_row M2 "it names every file in the deliverable list, and each named path exists"
declare_row M3 "the invocation lines it prints are runnable verbatim: --offline copied out exits 0"
declare_row M4 "EVAL_MODEL and EVAL_MAX_TURNS are documented with the defaults the script reads"
declare_row M5 "it states in plain words that the eval is not in task check, and gives the reason"
declare_row M6 "both follow-ups are named, each with its reason"
declare_row M7 "it describes no assertion the runner does not make"

[ -f "$README" ] || { red_all "no README at ${README#"$REPO"/}"; finish; exit; }

# The section: from its own top-level heading to the next one. The two siblings are `# Plugin eval`
# and `# Driven eval`, so this one is a `#` heading too — which is M1's first claim.
SECTION="$(awk '/^# .*[Aa]gent/ { inside = 1; print; next }
  inside && /^# / { inside = 0 }
  inside { print }' "$README")"

# ---- M1 -----------------------------------------------------------------------------------------
# "Shown by placing the three side by side": the comparison is the sibling sections' own shape —
# a `#` heading, then `##` subsections, and a fenced invocation block near the top.
R=0
if [ -z "$SECTION" ]; then
  R=1; why "no top-level section for the agent eval in ${README#"$REPO"/}"
else
  SIB_SUBS="$(awk '/^# (Plugin|Driven) eval/ { inside = 1; next }
    inside && /^# / { inside = 0 }
    inside && /^## / { print }' "$README" | sort -u | wc -l)"
  OWN_SUBS="$(grep -c '^## ' <<< "$SECTION")"
  [ "$OWN_SUBS" -ge 2 ] \
    || { R=1; why "the section has $OWN_SUBS \`##\` subsection(s); its siblings carry $SIB_SUBS between them"; }
  grep -q '^```' <<< "$SECTION" || { R=1; why "the section carries no fenced invocation block"; }
  grep -qE '^\| .* \|$' <<< "$SECTION" || { R=1; why "the section carries no file table, as both siblings do"; }
fi
row M1 "$R"

# ---- M2 -----------------------------------------------------------------------------------------
# Two claims, and the second is the one that rots: a path named in prose that nobody ever creates.
R=0
while IFS= read -r rel; do
  base="${rel#eval/}"
  grep -qF "$base" <<< "$SECTION" || { R=1; why "the section does not name $rel"; }
  [ -e "$PLUGIN_DIR/$rel" ] || { R=1; why "$rel is named but does not exist"; }
done < <(contract_lines deliverables.txt)
row M2 "$R"

# ---- M3 -----------------------------------------------------------------------------------------
# Runnable verbatim, run verbatim. The line is taken out of the fenced block and executed, because
# an invocation nobody has run is documentation of an intention.
R=0
LINE="$(grep -oE '(\./)?run-agents\.sh --offline[^`]*' <<< "$SECTION" | head -1)"
if [ -z "$LINE" ]; then
  R=1; why "the section prints no \`run-agents.sh --offline\` invocation"
elif [ ! -f "$RUNNER" ]; then
  R=1; why "$RUNNER does not exist, so the printed invocation cannot be run"
else
  ( cd "$EVAL_DIR" && eval "bash $LINE" ) > /dev/null 2>&1 \
    || { R=1; why "the invocation the README prints exited non-zero: $LINE"; }
fi
row M3 "$R"

# ---- M4 -----------------------------------------------------------------------------------------
# "Compared against the script, not from memory." The default is read out of the script's own
# parameter expansion, which is where a default that drifted would drift.
R=0
if [ ! -f "$RUNNER" ]; then
  R=1; why "$RUNNER does not exist, so no default can be compared against it"
else
  for var in EVAL_MODEL EVAL_MAX_TURNS; do
    DEFAULT="$(grep -oE "\\\$\{$var:-[^}]*\}" "$RUNNER" | head -1 | sed "s/.*:-//; s/}//")"
    if [ -z "$DEFAULT" ]; then
      R=1; why "$RUNNER reads no default for $var"
      continue
    fi
    grep -qF "$var" <<< "$SECTION" || { R=1; why "the section does not document $var"; }
    grep -qF "$DEFAULT" <<< "$SECTION" \
      || { R=1; why "the section does not give $var's actual default ($DEFAULT)"; }
  done
fi
row M4 "$R"

# ---- M5 -----------------------------------------------------------------------------------------
R=0
grep -qiE 'not (a )?part of `?task check`?|never part of `?task check`?|not in `?task check`?' <<< "$SECTION" \
  || { R=1; why "the section does not say in plain words that the eval is outside \`task check\`"; }
grep -qiE 'api|network|money|costs?' <<< "$SECTION" \
  || { R=1; why "the section gives no reason — the live mode reaches the API: network and money"; }
row M5 "$R"

# ---- M6 -----------------------------------------------------------------------------------------
# Both, each with its reason. A follow-up named without one is a note nobody can act on, and the
# specification records both reasons already.
R=0
grep -qF 'task agent-eval' <<< "$SECTION" || { R=1; why "the \`task agent-eval\` follow-up is not named"; }
grep -qF 'trace-spec' <<< "$SECTION" || { R=1; why "the \`cargo test -p trace-spec\` follow-up is not named"; }
grep -qiE 'surface|outside|constraint|crates/' <<< "$SECTION" \
  || { R=1; why "neither follow-up carries its reason (the declared implementation surface)"; }
row M6 "$R"

# ---- M7 -----------------------------------------------------------------------------------------
# Every row id the section mentions must be one the runner's table actually carries. The direction
# matters: a README may say less than the runner does, never more.
R=0
KNOWN=" "
while IFS=$'\t' read -r _ id _; do KNOWN="$KNOWN$id "; done < <(contract_lines verdict-rows.txt)
while IFS=$'\t' read -r _ id _ _ _ _; do KNOWN="$KNOWN$id "; done < <(contract_lines trace-expectations.txt)
MENTIONED="$(grep -oE '\b[DP][0-9]{1,2}\b' <<< "$SECTION" | sort -u)"
while IFS= read -r id; do
  [ -z "$id" ] && continue
  [[ "$KNOWN" == *" $id "* ]] || { R=1; why "the section names row $id, which no runner row carries"; }
done <<< "$MENTIONED"
# The other half: a promise written as prose rather than as an id.
grep -qiE 'assert|check|verif' <<< "$SECTION" \
  || { R=1; why "the section describes no assertion at all — M7 has nothing to compare"; }
row M7 "$R"

finish
