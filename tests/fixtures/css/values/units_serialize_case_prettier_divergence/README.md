# units_serialize_case (prettier divergence)

CSS dimension units are ASCII case-insensitive (CSS Syntax 3) and their **canonical
serialized form is lowercase** — including the frequency units `Hz`/`kHz` and the
quarter-millimeter `Q`:

- CSS Values 4 §7.3: _"All `<frequency>` units are compatible, and `hz` is their
  canonical unit"_; _"Units … serialize as lowercase, for example 1Hz serializes as
  1hz."_
- CSS Values 4 §6.2: _"… 1Q serializes as 1q."_

tsv canonicalizes every unit to this lowercase serialized form (`440Hz` → `440hz`,
`1kHz` → `1khz`, `10Q` → `10q`), the same rule it already applies to `PX` → `px`.

**Prettier** instead **upcases** these three to their prose spelling (`440hz` →
`440Hz`, `1khz` → `1kHz`, `10q` → `10Q`) — see `output_prettier.svelte`. Every other
unit lowercases in both formatters; this divergence is only the `Hz`/`kHz`/`Q` trio.

tsv follows the spec's canonical form here (spec precedence over prettier), so the
units stay lowercase.

See [conformance_prettier.md §Unit serialization case](../../../../../docs/conformance_prettier.md#css-values).
