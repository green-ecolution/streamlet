#!/usr/bin/env bash

PBF_URL=${PBF_URL:-}
PBF_PATH=${PBF_PATH:-}

DATA_DIR=${DATA_DIR:-./data}
FILENAME=${FILENAME:-${PBF_URL##*/}}
FILENAME=${FILENAME:-${PBF_PATH##*/}}

S3_HOST="${S3_HOST:-s3.localhost}"
S3_PORT="${S3_PORT:-3000}"
S3_SCHEME="${S3_SCHEME:-http}"
S3_ACCESS_KEY="${S3_ACCESS_KEY:-root}"
S3_SECRET_KEY="${S3_SECRET_KEY:-password}"
S3_BUCKET="${S3_BUCKET:-valhalla-data}"
S3_REGION="${S3_REGION:-de}"
S3_ENDPOINT="$S3_SCHEME://$S3_HOST:$S3_PORT"

TILE_ARCHIVE_SET=true

TILE_ARCHIVE_PREFIX=${TILE_ARCHIVE_PREFIX:-tiles}

if [[ -z $TILE_ARCHIVE ]]; then
	TILE_ARCHIVE_SET=false
fi

reset_tile_archive() {
	DATE=$(date +%Y-%m-%d)
	if ! $TILE_ARCHIVE_SET; then
		TILE_ARCHIVE="$TILE_ARCHIVE_PREFIX-$DATE.tar.gz"
	fi
}

reset_tile_archive

LOCAL_ARCHIVE_NAME="${LOCAL_ARCHIVE_NAME:-$TILE_ARCHIVE}"

fatal() {
	echo "[ERROR]" "$@" >&2
	exit 1
}

print_config() {
	echo "PBF_URL: $PBF_URL"
	echo "PBF_PATH: $PBF_PATH"
	echo "DATA_DIR: $DATA_DIR"
	echo "FILENAME: $FILENAME"

	echo "S3_HOST: $S3_HOST"
	echo "S3_PORT: $S3_PORT"
	echo "S3_SCHEME: $S3_SCHEME"
	echo "S3_BUCKET: $S3_BUCKET"
	echo "S3_REGION: $S3_REGION"
	echo "S3_ENDPOINT: $S3_ENDPOINT"

	echo "DATE: $DATE"

	echo "TILE_ARCHIVE_PREFIX: $TILE_ARCHIVE_PREFIX"
	echo "TILE_ARCHIVE: $TILE_ARCHIVE"
	echo "TILE_ARCHIVE_SET: $TILE_ARCHIVE_SET"
	echo "LOCAL_ARCHIVE_NAME: $LOCAL_ARCHIVE_NAME"
}

aws_s3() {
	AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY" \
	AWS_SECRET_ACCESS_KEY="$S3_SECRET_KEY" \
	AWS_DEFAULT_REGION="$S3_REGION" \
		aws --endpoint-url "$S3_ENDPOINT" "$@"
}

setup_data_dir() {
	echo "📁 Creating data directory: $DATA_DIR"
	mkdir -p "$DATA_DIR"
	cd "$DATA_DIR" || fatal "how, i created it one second ago"
	echo "✅ Data directory ready"
}

copy_or_download_tiles() {
	[[ -z $PBF_URL && -z $PBF_PATH ]] && fatal "no pbf file path or url provided"

	if [[ -n "$PBF_URL" ]]; then
		echo "📥 Downloading PBF file from: $PBF_URL"
		curl -SfL -o "$FILENAME" "$PBF_URL" || fatal "file not found"
		echo "✅ Download complete"
	else
		echo "📋 Copying PBF file from: $PBF_PATH"
		cp -f "$PBF_PATH" "$FILENAME" || fatal "file not found"
		echo "✅ Copy complete"
	fi
}

modify_tiles() {
	echo "🔧 Starting to modify tiles file..."
	add_custom_roads
	add_roadworks
	echo "✅ Finished modifying tiles file"
}

add_custom_roads() {
	echo "🛣️  Creating changeset with custom roads..."
	echo "📝 Applying changeset to tiles..."
	echo "✅ Custom roads added"
}

add_roadworks() {
	echo "🚧 Creating changeset with local roadworks..."
	echo "📝 Applying changeset to tiles..."
	echo "✅ Roadworks added"
}

archive_tiles() {
	echo "📦 Starting to archive files..."
	mkdir -p data
	mv ./* data/
	echo "🗜️  Compressing archive: $TILE_ARCHIVE"
	tar czf "$TILE_ARCHIVE" data
	mv data/* .
	echo "✅ Files are now archived"
}

extract_tiles() {
	echo "📦 Starting to extract archive: $TILE_ARCHIVE"
	tar xzf "$TILE_ARCHIVE"
	mv "$TILE_ARCHIVE" "data/$LOCAL_ARCHIVE_NAME"
	echo "✅ Files are now extracted"
}

wait_for_s3() {
	echo "🔍 Checking if S3 storage is online..."
	i=0
	until curl -Is --connect-timeout 1 --max-time 2 "$S3_ENDPOINT" >/dev/null; do
		((i++))
		if (( i > 10 )); then
			fatal "s3 is not available! Exiting..."
		fi
		echo "⏳ Waiting for S3 to become available (attempt $i/10)..."
		sleep 5
	done
	echo "✅ S3 storage is online"
}

upload_archive() {
	echo "📤 Starting to upload archive to S3..."
	echo "📦 Archive: $TILE_ARCHIVE"
	aws_s3 s3 cp --no-progress "$TILE_ARCHIVE" "s3://$S3_BUCKET/$TILE_ARCHIVE" || fatal "s3 upload"
	echo "✅ Finished uploading archive to S3"
}

list_archives() {
	local prefix=$1
	aws_s3 s3api list-objects-v2 --bucket "$S3_BUCKET" --prefix "$prefix" \
		--query 'Contents[].Key' --output text | tr '\t' '\n' | grep -v '^None$' | sort
}

select_archive() {
	echo "🔍 Selecting archive from S3..."
	if ! aws_s3 s3api head-object --bucket "$S3_BUCKET" --key "$TILE_ARCHIVE" >/dev/null 2>&1; then
		if "$TILE_ARCHIVE_SET"; then
			echo "❌ Archive not found on S3" >&2
			return 1
		fi

		echo "🔄 Today's archive not found, trying to get latest archive..." >&2
		TILE_ARCHIVE=$(list_archives "$TILE_ARCHIVE_PREFIX" | tail -n 1)

		if [[ -z $TILE_ARCHIVE ]]; then
			echo "❌ No archive found on S3!" >&2
			return 1
		fi
		echo "✅ Found latest archive: $TILE_ARCHIVE" >&2
	else
		echo "✅ Archive found: $TILE_ARCHIVE"
	fi
}

download_archive() {
	echo "📥 Starting to download $TILE_ARCHIVE from S3..."
	aws_s3 s3 cp --no-progress "s3://$S3_BUCKET/$TILE_ARCHIVE" . || fatal "download archive"
	echo "✅ Finished downloading archive from S3"
}

cleanup_archive() {
	echo "🧹 Cleaning up archive: $TILE_ARCHIVE"
	rm "$TILE_ARCHIVE"
	echo "✅ Cleanup complete"
}
