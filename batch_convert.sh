#!/bin/bash
# Batch conversion helper script
#
# Create a new directory with two child directories (ssq and xwb). Put the step
# charts (*.ssq) into ssq and the wave banks (*.xwb) into xwb. If the directory
# is a child of this project, you can directly run the script, otherwise you
# have to execute it with the environment variable RUN_COMMAND that points to
# the executable. If you want to specify additional flags to pass to brd you
# can just pass them to this script. The converted beatmaps will be placed in
# the osz directory.
# If you want your beatmaps to have the right metadata (title and artist),
# create a file “metadata.csv” in the directory where you run this script with
# the following structure: name,Title,Artist (name is the name of the ssq file
# without the extension)
#
# Example:
# $ RUN_COMMAND=/path/to/target/release/brd /path/to/batch_covert.sh --source "Dance Dance Revolution x3"

set -e

mkdir -p osz

RUN_COMMAND=${RUN_COMMAND:-"cargo run --release --"}

echo "Extracting wave bank sound names"
for i in xwb/*.xwb; do
	outfile="xwb/$(basename $i .xwb).xwb.sounds"
	if ! [ -f "$outfile" ]; then
		$RUN_COMMAND unxwb -l $i > "$outfile"
	fi
done

for ssq_file in ssq/*.ssq; do
	name=$(basename $ssq_file .ssq)

	if [ -f "xwb/${name}.xwb" ]; then
		xwb_file="xwb/${name}.xwb"
	else
		xwb_file="$(grep -lE "^${name}$" xwb/*.xwb.sounds|head -n 1)"
		# strip .sounds
		xwb_file="${xwb_file%.*}"
		if [ -z "$xwb_file" ]; then
			echo "ERR: Could not find wave bank for $name" >&2
			continue
		fi
	fi

	metadata=$(grep -sE "^${name}," metadata.csv|head -n 1)
	if ! [ -z "$metadata" ]; then
		title="$(cut -d, -f2 <<< $metadata)"
		artist="$(cut -d, -f3 <<< $metadata)"
	else
		title="$name"
		artist="unknown artist"
	fi

	echo "Converting $name"
	$RUN_COMMAND ddr2osu -s "$ssq_file" -x "$xwb_file" -o "osz/${name}.osz" --title "$title" --artist "$artist" "$@"
done
