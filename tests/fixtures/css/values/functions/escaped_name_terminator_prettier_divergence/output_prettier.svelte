<style>
	a {
		/* the author glued the name to its `(`: a hex escape's terminator is optional and
		`(` is no hex digit, so the ident sequence ends there either way and the run is the
		same function token — tsv keeps the spelling it was given */
		color: c\41 (0.1);
		/* the escape is not even last here, so nothing needs terminating */
		border-color: c\41x (0.1);
		/* prettier's insertion does not fire on a name carrying an interior `-` */
		outline-color: c\41-x(0.1);
	}
</style>
