<script>
	// A member-only chain (no calls) whose base breaks INTERNALLY. The lookup fits on the
	// base's closing line, so it hugs `]` / `}` / `)` instead of dropping to its own line:
	// each lookup is measured in its own group, so a broken base does not force it down.
	// The last two cases are controls for the other direction.

	// array base breaks, `.length` hugs `]`
	const a = [
		alphaValue,
		bravoValue,
		charlieValue,
		deltaValue,
		echoValue,
		foxtrotValue,
		golfValue,
		hotelValue
	].length;

	// object base breaks, `.alphaKey` hugs `}`
	const b = {
		alphaKey: 1,
		bravoKey: 2,
		charlieKey: 3,
		deltaKey: 4,
		echoKey: 5,
		foxtrotKey: 6,
		golfKey: 7
	}.alphaKey;

	// JSDoc cast base breaks, `.css` hugs `)`
	const c = /** @type {{ css: { children: StyleSheetChildren } }} */ (
		svelte.parse(`<style>${css}</style>`)
	).css;

	// optional lookup after a breaking cast base
	const d = /** @type {{ css: { children: StyleSheetChildren } }} */ (
		svelte.parse(`<style>${css}</style>`)
	)?.css;

	// two lookups ride the closing line together
	const e = [
		alphaValue,
		bravoValue,
		charlieValue,
		deltaValue,
		echoValue,
		foxtrotValue,
		golfValue,
		hotelV
	].length.toFixed;

	// CONTROL: the base stays on one line and the whole expression overflows, so the
	// lookup DOES drop to its own line
	const f = { alpha: 1, bravo: 2, charlie: 3, delta: 4, echo: 5, foxtrot: 6, golf: 7, hotel: 88 }
		.alpha;

	// CONTROL: a call base whose args break already hugs
	const g = aaaa.bbbb.cccc({ prop: 'value' }).length;
</script>
