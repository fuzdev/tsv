<script lang="ts">
	// Fits on one line at 100 — the mapped type-arg stays on the `=` line.
	const a = fnnnnnnnnnnnnnnnnnn<{ [K in keyof T]: number | string | null | undefined | boolean }>();

	// At 101 the line breaks after `=`; the width-breakable mapped type-arg stays inline.
	const b =
		fnnnnnnnnnnnnnnnnnnn<{ [K in keyof T]: number | string | null | undefined | boolean }>();

	// After the `=` break the call is exactly 100 at its indent — the mapped still stays inline.
	const c =
		fnnnnnnnnnnnnnnnnnnnnnnnnnn<{ [K in keyof T]: number | string | null | undefined | boolean }>();

	// One char longer — the call no longer fits even alone, so the mapped wraps and `=` stays inline.
	const d = fnnnnnnnnnnnnnnnnnnnnnnnnnnn<{
		[K in keyof T]: number | string | null | undefined | boolean;
	}>();
</script>
