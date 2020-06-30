# BRD

BRD is a tool for working with [*Dance Dance Revolution*][ddr] related data.
For currently supported features, see the [Modes](#modes) section.

## Installation

Currently this is not published as a crate so you either have to clone the
repository manually and run `cargo build --release` or you can use `cargo
install --git https://github.com/sbruder/brd` to install the binary without
cloning.

## Modes

### ddr2osu

This converts DDR step charts (.ssq files) and the corresponding audio (from
.xwb files) to osu beatmaps (in an .osz container).

Basic usage:

```shell
brd ddr2osu -s file.ssq -x file.xwb -o file.osz --title "Song Title" --artist "Song Artist"
# use musicdb from DDR A
brd ddr2osu -s file.ssq -x file.xwb -o file.osz -m startup.arc
```

To learn more about supported options run `brd ddr2osu --help`

Batch conversion is possible with the included shell script `batch_convert.sh`
(usage guide at the top of the script).

#### Known Problems

 * Since *osu!mania* does not support shock arrows, it either ignores them or
   (by default) replaces them with a two-key combination (↑↓ or ←→); you can
   change this with the (`--shock-action` option)
 * Known problems listed for unxwb (for wave banks without entry names having
   2 entries, which often are preview and full song, the longest one is used by
   default).

### unxwb

This can list and extract sounds from XWB wave banks.

Basic Usage:

    brd unxwb file.xwb
    brd unxwb -l file.xwb

### unarc

This can list and extract files from DDR A ARC archives. It extracts the
contents into the current directory.

Basic Usage:

    brd unarc file.arc
    brd unarc -l file.arc

### musicdb

This lists all entries from `musicdb.xml` or `startup.arc` files (only DDR A is
supported).

#### Known Problems

 * It only supports sounds in [ADPCM][ADPCM] format. If you want to extract
   sounds that are stored in other formats, you can use [Luigi Auriemma’s
   unxwb][unxwb] (<kbd>Ctrl</kbd>+<kbd>F</kbd> unxwb).
 * For wave banks without name entries it does not yet offer the option to read
   the names from [XSB files][multimedia.cx-XSB] and currently generates the
   names from the index in the file (starting from 0).

## About this project

This is my first rust project. Don’t expect too much from the code in terms of
quality, robustness or idiomacity (especially regarding error handling). There
currently are no tests for the actual functionality.

Large portions of this tool would not have been possible without the following
resources:

 * [SaxxonPike][SaxxonPike]’s [scharfrichter][scharfrichter] which implements
   [SSQ][scharfrichter-ssq] and XWB ([1][scharfrichter-xwb1],
   [2][scharfrichter-xwb2]) and their [documentation about SSQ][ssq-doc]
 * The [official osu! file format documentation][osu-doc]
 * [MonoGame][MonoGame]’s [XWB implementation][MonoGame-xwb]
 * [Luigi Auriemma][aluigi]’s [unxwb][unxwb] (especially the ADPCM header part)
 * [mon][mon]’s [ddr\_arc\_extract][ddr_arc_extract]

## License

[ISC License](LICENSE)

This project is not affiliated with ppy or Konami.

[ADPCM]: https://en.wikipedia.org/wiki/Adaptive_differential_pulse-code_modulation
[MonoGame-xwb]: https://github.com/MonoGame/MonoGame/blob/develop/MonoGame.Framework/Audio/Xact/WaveBank.cs
[MonoGame]: https://github.com/MonoGame/MonoGame
[SaxxonPike]: https://github.com/SaxxonPike
[aluigi]: http://aluigi.altervista.org/
[ddr]: https://en.wikipedia.org/wiki/Dance_Dance_Revolution
[ddr_arc_extract]: https://github.com/mon/ddr_arc_extract
[mon]: https://github.com/mon
[multimedia.cx-XSB]: https://wiki.multimedia.cx/index.php/XACT#Sound_Banks_.28.xsb.29
[osu!mania]: https://osu.ppy.sh/help/wiki/Game_Modes/osu%21mania
[osu-doc]: https://osu.ppy.sh/help/wiki/osu!_File_Formats/Osu_(file_format)
[scharfrichter-ssq]: https://github.com/SaxxonPike/scharfrichter/blob/master/Scharfrichter/Archives/BemaniSSQ.cs
[scharfrichter-xwb1]: https://github.com/SaxxonPike/scharfrichter/blob/master/Scharfrichter/Archives/MicrosoftXWB.cs
[scharfrichter-xwb2]: https://github.com/SaxxonPike/scharfrichter/blob/master/Scharfrichter/XACT3/Xact3WaveBank.cs
[scharfrichter]: https://github.com/SaxxonPike/scharfrichter
[ssq-doc]: https://github.com/SaxxonPike/rhythm-game-formats/blob/master/ddr/ssq.md
[unxwb]: http://aluigi.altervista.org/papers.htm
