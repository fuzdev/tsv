<style>
	/* An escaped pseudo-class name dispatches to its argument grammar by the DECODED
	   name, not the raw source: \69s decodes to `is`, so the argument is a forgiving
	   selector list. The escape is preserved in the printed name (recovered from source). */
	:\69s(.class) {
		color: red;
	}
	/* \6e\74\68-child decodes to `nth-child`, so the argument parses as an An+B `Nth`
	   (not a selector list) — the observable proof that dispatch reads the decoded name. */
	:\6e\74\68-child(2n + 1) {
		color: blue;
	}
</style>
