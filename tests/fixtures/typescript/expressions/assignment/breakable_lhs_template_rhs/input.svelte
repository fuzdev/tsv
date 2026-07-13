<script>
	// An unbreakable RHS (template literal) may only stay welded to the `=` when the LHS
	// has nowhere to break either — prettier chooseLayout, gated on `canBreakLeftDoc`.
	// Otherwise the assignment goes fluid and breaks AFTER the `=`, rather than letting
	// the overflow split the assignment target.

	// LHS can break (a computed lookup is a breakable group) -> break after `=`
	params['redirectTo'] =
		`${window.location.pathname}${window.location.search}${window.location.hash}`;

	// Same, with the lookup deeper in the chain
	params['redirectTo']['codePointAt']['name'] =
		`${window.location.pathname}${window.location.search}`;

	// A call in the LHS is breakable too
	document.querySelector('p').textContent =
		`Shader program did not link successfully: ${linkErrLog}`;

	// LHS cannot break (a plain identifier) -> the template stays welded and overflows
	paramsAaaaaaaaaaaaaaaaaaaaa = `${window.location.pathname}${window.location.search}${window.location.hash}`;
</script>
