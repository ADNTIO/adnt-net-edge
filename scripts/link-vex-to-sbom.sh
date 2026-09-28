#!/usr/bin/env bash
set -euo pipefail

if (($# != 3)); then
  echo "Usage: $0 SBOM.cdx.json VEX.cdx.json OUTPUT.cdx.json" >&2
  exit 2
fi

jq --slurpfile bom "$1" '
  ($bom[0].serialNumber | sub("^urn:uuid:"; "")) as $serial
  | ($bom[0].version // 1) as $version
  | def bom_ref($purl):
      ([ $bom[0].components[]? | select(.purl == $purl) | .["bom-ref"] ][0]) as $ref
      | if $ref == null then error("VEX PURL is absent from the SBOM: \($purl)") else $ref end;
  .vulnerabilities |= map(
    .affects |= map(
      .ref as $purl
      | bom_ref($purl) as $ref
      | .ref = "urn:cdx:\($serial)/\($version)#\($ref)"
    )
  )
' "$2" > "$3"
