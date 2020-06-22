# BRD

BRD is a tool for working with [*Dance Dance Revolution*][ddr] step charts and
wave banks. It currently supports conversion to [*osu!mania*][osu!mania]
beatmaps and extraction of sounds from wave banks.

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

    brd ddr2osu -s file.ssq -x file.xwb -o file.osz --title "Song Title" --artist "Song Artist"

To learn more about supported options run `brd ddr2osu --help`

Batch conversion is possible with the included shell script `batch_convert.sh`
(usage guide at the top of the script).

#### Known Problems

 * Since *osu!mania* does not support shock arrows, it either ignores them or
   (by default) replaces them with a two-key combination (↑↓ or ←→); you can
   change this with the (`--shock-action` option)
 * Freezes do not work (I do not know how to get the start time yet) and
   therefore are disabled in code (`ddr::ssq::FREEZE`)

### unxwb

This can list and extract sounds from XWB wave banks. It currently only
supports [ADPCM][ADPCM] sounds.
Basic Usage:

    brd unxwb file.xwb
    brd unxwb -l file.xwb

#### Known Problems

If you want to extract sounds that are stored in other formats, you can use
[Luigi Auriemma’s unxwb][unxwb] (<kbd>Ctrl</kbd>+<kbd>F</kbd> unxwb).

## About this project

This is my first rust project. Don’t expect too much from the code in terms of
quality, robustness or idiomacity (especially regarding error handling). There
currently are no tests.

Large portions of this tool would not have been possible without the following
resources:

 * [SaxxonPike][SaxxonPike]’s [scharfrichter][scharfrichter] which implements
   [SSQ][scharfrichter-ssq] and XWB ([1][scharfrichter-xwb1],
   [2][scharfrichter-xwb2]) and their [documentation about SSQ][ssq-doc]
 * The [official osu! file format documentation][osu-doc]
 * [MonoGame][MonoGame]’s [XWB implementation][MonoGame-xwb]
 * [Luigi Auriemma][aluigi]’s [unxwb][unxwb] (especially the ADPCM header part)

## License

[ISC License](LICENSE)

This project is not affiliated with ppy or Konami.

[ADPCM]: https://en.wikipedia.org/wiki/Adaptive_differential_pulse-code_modulation
[MonoGame-xwb]: https://github.com/MonoGame/MonoGame/blob/develop/MonoGame.Framework/Audio/Xact/WaveBank.cs
[MonoGame]: https://github.com/MonoGame/MonoGame
[SaxxonPike]: https://github.com/SaxxonPike
[aluigi]: http://aluigi.altervista.org/
[ddr]: https://en.wikipedia.org/wiki/Dance_Dance_Revolution
[osu!mania]: https://osu.ppy.sh/help/wiki/Game_Modes/osu%21mania
[osu-doc]: https://osu.ppy.sh/help/wiki/osu!_File_Formats/Osu_(file_format)
[scharfrichter-ssq]: https://github.com/SaxxonPike/scharfrichter/blob/master/Scharfrichter/Archives/BemaniSSQ.cs
[scharfrichter-xwb1]: https://github.com/SaxxonPike/scharfrichter/blob/master/Scharfrichter/Archives/MicrosoftXWB.cs
[scharfrichter-xwb2]: https://github.com/SaxxonPike/scharfrichter/blob/master/Scharfrichter/XACT3/Xact3WaveBank.cs
[scharfrichter]: https://github.com/SaxxonPike/scharfrichter
[ssq-doc]: https://github.com/SaxxonPike/rhythm-game-formats/blob/master/ddr/ssq.md
[unxwb]: http://aluigi.altervista.org/papers.htm
