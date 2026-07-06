#!/usr/bin/env bash

PBF_URL=${PBF_URL:-https://download.geofabrik.de/europe/germany/schleswig-holstein-latest.osm.pbf}
DATA_DIR=${DATA_DIR:-./data}
FILENAME=${FILENAME:-${PBF_URL##*/}}
OUTPUT_PATH=${OUTPUT_PATH:-./output}
OUTPUT_FILENAME=${OUTPUT_FILENAME:-${FILENAME/latest/changed}}
OUTPUT="$OUTPUT_PATH/$OUTPUT_FILENAME"
BOUNDING_BOX=${BOUNDING_BOX:-9.357298,54.751799,9.506812,54.837072}
BOUNDED_FILENAME=${BOUNDED_FILENAME:-${FILENAME/schleswig-holstein/flensburg}}
SKIP_CONSTRUCTION=${SKIP_CONSTRUCTION:-false}

CHANGESETS=()

fatal() {
	echo "[ERROR]" "$@" >&2
	exit 1
}

print_config() {
	echo "PBF_URL: $PBF_URL"
	echo "DATA_DIR: $DATA_DIR"
	echo "FILENAME: $FILENAME"
	echo "OUTPUT: $OUTPUT"
	echo "BOUNDING_BOX: $BOUNDING_BOX"
	echo "BOUNDED_FILENAME: $BOUNDED_FILENAME"
	echo "SKIP_CONSTRUCTION: $SKIP_CONSTRUCTION"
}

setup() {
	mkdir -p "$DATA_DIR"
	cd "$DATA_DIR" || fatal "failed to change to data directory: $DATA_DIR"
	mkdir -p "$OUTPUT_PATH" || fatal "failed to create output directory: $OUTPUT_PATH"
}

download_pbf() {
	echo "Downloading PBF from: $PBF_URL"
	curl --connect-timeout 10 --max-time 300 -SfL -o "$FILENAME" "$PBF_URL" \
		|| fatal "failed to download PBF from $PBF_URL"
}

extract_bounding_box() {
	echo "Extracting bounding box: $BOUNDING_BOX"
	osmium extract -b "$BOUNDING_BOX" "$FILENAME" --overwrite -o "$BOUNDED_FILENAME" \
		|| fatal "failed to extract bounding box"
}

empty_changeset() {
	cat <<- 'EOF' > "$1"
		<?xml version="1.0" encoding="UTF-8"?>
		<osmChange version="0.6" generator="empty">
		</osmChange>
	EOF
	CHANGESETS+=("$1")
}

create_construction_changeset() {
	if [[ $SKIP_CONSTRUCTION != "false" ]]; then
		echo "Construction changeset skipped"
		return
	fi
	echo "Creating construction changeset..."
	/pbf-patch construction --input "$BOUNDED_FILENAME" --output construction.osc \
		|| fatal "failed to create construction changeset"
	CHANGESETS+=("construction.osc")
}

apply_changesets() {
	echo "Applying ${#CHANGESETS[@]} changeset(s)..."
	osmium apply-changes --overwrite -o "$OUTPUT" "$FILENAME" "${CHANGESETS[@]}" \
		|| fatal "failed to apply changesets"
}

main() {
	if [[ -n $DEBUG ]]; then
		print_config
	fi

	setup
	download_pbf
	extract_bounding_box
	empty_changeset "empty.osc"
	create_construction_changeset
	apply_changesets

	echo "Output file: $OUTPUT"
}

main "$@"
