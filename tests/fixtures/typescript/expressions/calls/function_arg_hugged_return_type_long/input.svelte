<script lang="ts">
	// The return type's first break sits inside a sole type argument's object type: the callback
	// keeps the hug, measured to that `{`
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{
		x: X;
	}> {
		g(a);
	});
	fn(lead, async function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{
		x: X;
	}> {
		g(a);
	});

	// Where the broken-out line cannot hold the parameter flat either, the hug measured to the type
	// argument's `{` still comes first
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{
		x: X;
	}> {
		g(a);
	});

	// The same inside a hugged union with `null`
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): {
		x: X;
	} | null {
		g(a);
	});

	// At 100 the call fits on one line up to the body's `{`, so it stays flat
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{ x: X }> {
		g(a);
	});

	// At 101 it does not: the callback keeps the hug, measured to the type argument's `{`
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{
		x: X;
	}> {
		g(a);
	});

	// At 100 the hugged line up to the type argument's `{` fits, so the callback keeps the hug
	fn(leadingArgument, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{
		x: X;
	}> {
		g(a);
	});

	// At 101 it does not, so the arguments break out (a longer first argument keeps the parameter
	// flat on the broken-out line)
	fn(
		leadingArgument,
		function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{ x: X }> {
			g(a);
		}
	);

	// In a hugged union: at 100 the hugged line up to the first member's `{` fits
	fn(leadingArgument, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): {
		x: X;
	} | null {
		g(a);
	});

	// At 101 it does not, so the arguments break out
	fn(
		leadingArgument,
		function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } | null {
			g(a);
		}
	);

	// A paren shell around the type argument's object type prints stripped, so the hug is measured
	// to that `{` as without it
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): Promise<{
		y: Y;
	}> {
		g(a);
	});

	// The same around a hugged union's object member
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): {
		y: Y;
	} | null {
		g(a);
	});

	// A callback with no parameters: at 100 the call fits on one line up to the body's `{`
	fn(leadingArgumentEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE, function (): Promise<{ x: X }> {
		g();
	});

	// At 101 it does not: the callback keeps the hug, measured to the type argument's `{`
	fn(leadingArgumentEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE, function (): Promise<{
		x: X;
	}> {
		g();
	});
</script>
