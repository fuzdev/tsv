<style>
	/* In a pseudo name, prettier lowercases everything up to the first whitespace
	   (a hex escape's terminator) and preserves the rest. So escape hex digits and
	   leading literals fold, but literal letters after a terminator keep their case.
	   This differs from class/id/type selectors, which never fold (see escaped_names). */

	/* hex escape digits fold to lowercase */
	:\4a b {
		color: red;
	}
	/* pseudo-elements fold the same way */
	::\4a b {
		color: blue;
	}
	/* a literal letter after the escape's terminator keeps its case */
	:\6e OT {
		color: green;
	}
	/* an escaped leading dash is not a custom `:--` pseudo, so the name folds */
	:\2d-custom {
		color: orange;
	}
</style>
